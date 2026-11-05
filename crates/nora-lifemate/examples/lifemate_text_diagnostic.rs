//! Train-development causal diagnostic only. Reuses the real planner and SDK
//! with supplied transcripts to isolate ASR from planning errors. Results are
//! NOT voice accuracy, latency benchmarks, or contest submissions.
use nora_lifemate::{
    Result,
    local::{LocalModel, ModelAssets, ModelMode, ModelRuntime},
    protocol::{InputItem, audio_path},
    runner::{self, RunConfig, Speech, TrainPartition},
};
use serde::Deserialize;
#[cfg(test)]
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transcripts {
    scope: String,
    by_audio: BTreeMap<String, String>,
}
struct SuppliedSpeech(BTreeMap<PathBuf, String>);
impl Speech for SuppliedSpeech {
    fn transcribe(&mut self, path: &Path, deadline: Instant) -> Result<String> {
        if Instant::now() >= deadline {
            return Err("diagnostic deadline exceeded".into());
        }
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| "transcript absent from selected development audio".into())
    }
}

fn runtime_options(args: &mut Vec<String>) -> Result<ModelRuntime> {
    let mut end = args.len();
    let cpu_output = end > 0 && args[end - 1] == "--cpu-output";
    if cpu_output {
        end -= 1;
    }
    let gpu_layers = if end >= 2 && args[end - 2] == "--gpu-layers" {
        end -= 2;
        args[end + 1].as_str()
    } else {
        "all"
    };
    let mut runtime = ModelRuntime::parse(true, gpu_layers)?.with_cpu_output(cpu_output);
    if end >= 3 && args[end - 3] == "--prefill-batch" {
        let offset = end - 3;
        runtime =
            runtime.with_prefill_batch(args[offset + 1].parse()?, args[offset + 2].parse()?)?;
        end = offset;
    }
    args.truncate(end);
    Ok(runtime)
}

/// A frozen local adapter is opt-in. Hash before any model process or output
/// directory is created; the supervisor also verifies assets after the run.
fn adapter_options(args: &mut Vec<String>) -> Result<Option<PathBuf>> {
    if args.len() < 3 || args[args.len() - 3] != "--lora" {
        return Ok(None);
    }
    let offset = args.len() - 3;
    let path = nora_lifemate::assets::pinned_lora(Path::new(&args[offset + 1]), &args[offset + 2])?;
    args.truncate(offset);
    Ok(Some(path))
}

fn semantic_options(args: &mut Vec<String>) -> Result<Option<[String; 4]>> {
    if args.len() >= 5 && args[args.len() - 5] == "--semantic-retrieval" {
        let offset = args.len() - 5;
        let options = args[offset + 1..]
            .to_vec()
            .try_into()
            .map_err(|_| "semantic retrieval requires four asset arguments")?;
        args.truncate(offset);
        if args.len() != 17
            && !(args.len() == 18 && args.last().is_some_and(|s| s == "--example-before-request"))
        {
            return Err("semantic retrieval requires pinned training examples and cohort".into());
        }
        Ok(Some(options))
    } else {
        Ok(None)
    }
}

fn presentation_options(args: &mut Vec<String>) -> Result<bool> {
    if args.last().is_some_and(|s| s == "--example-before-request") {
        args.pop();
        if args.len() != 17 {
            return Err("example presentation requires pinned training examples and cohort".into());
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

fn main() -> Result<()> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let adapter = adapter_options(&mut args)?;
    let runtime = runtime_options(&mut args)?;
    let semantic = semantic_options(&mut args)?;
    let example_before_request = presentation_options(&mut args)?;
    #[cfg(not(feature = "semantic-retrieval"))]
    if semantic.is_some() {
        return Err("semantic retrieval requires its explicit build feature".into());
    }
    if !matches!(args.len(), 9 | 12 | 13 | 17) {
        return Err(
            "expected ROOT SERVER MODEL PYTHON BRIDGE NEW_OUTPUT_DIR LIMIT MODE TRANSCRIPTS [COHORT SHA256 CONTEXT [DIAGNOSTIC_DEADLINE_MS [TRAIN_EXAMPLES SHA256 TRAIN_IDS SHA256]]] [--example-before-request] [--semantic-retrieval INDEX SHA256 ASSETS SHA256] [--prefill-batch BATCH MICROBATCH] [--gpu-layers all|0..256] [--cpu-output] [--lora ADAPTER SHA256]".into(),
        );
    }
    let root = PathBuf::from(&args[0]).canonicalize()?;
    let split = root.join("train").canonicalize()?;
    let limit: usize = args[6].parse()?;
    if !(1..=1235).contains(&limit) {
        return Err("diagnostic development sample bound".into());
    }
    let mode = ModelMode::parse(&args[7])?;
    let cohort = if args.len() >= 12 {
        let cohort =
            nora_lifemate::cohort::EvaluationCohort::from_file(Path::new(&args[9]), &args[10])?;
        if cohort.len() != limit {
            return Err("diagnostic cohort/count mismatch".into());
        }
        Some(cohort)
    } else {
        None
    };
    let context = if args.len() >= 12 {
        args[11].parse()?
    } else {
        16_384
    };
    let deadline_ms = args
        .get(12)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(30_000);
    let transcript_path = PathBuf::from(&args[8]);
    if transcript_path.metadata()?.len() > 8_000_000 {
        return Err("diagnostic transcripts exceed bound".into());
    }
    let supplied: Transcripts = serde_json::from_reader(File::open(&transcript_path)?)?;
    if !matches!(
        supplied.scope.as_str(),
        "train-development-reference-diagnostic" | "train-development-asr-diagnostic"
    ) {
        return Err("only train development diagnostics permitted".into());
    }
    let mut expected = BTreeMap::new();
    let mut count = 0;
    for line in BufReader::new(File::open(split.join("train.jsonl"))?).lines() {
        let item: InputItem = serde_json::from_str(&line?)?;
        if !TrainPartition::Development.accepts(&item.id) {
            continue;
        }
        if cohort.as_ref().is_some_and(|c| !c.contains(&item.id)) {
            continue;
        }
        item.validate()?;
        for audio in std::iter::once(item.audio.as_str()).chain(
            item.history
                .iter()
                .filter(|turn| turn.role == "user")
                .filter_map(|turn| turn.audio.as_deref()),
        ) {
            let text = supplied
                .by_audio
                .get(audio)
                .ok_or("missing selected transcript")?;
            if text.is_empty() || text.len() > 65_536 {
                return Err("invalid diagnostic transcript".into());
            }
            expected.insert(audio_path(&split, audio)?, text.clone());
        }
        count += 1;
        if count >= limit {
            break;
        }
    }
    if count != limit || expected.len() != supplied.by_audio.len() {
        return Err("diagnostic coverage crosses selected development audio".into());
    }
    let directory = PathBuf::from(&args[5]);
    std::fs::create_dir(&directory)?;
    std::fs::write(
        directory.join("DIAGNOSTIC_ONLY.txt"),
        format!(
            "{}\nNOT voice accuracy. NOT official results. Timing excludes ASR. Do not submit.\nDiagnostic deadline_ms={deadline_ms}; contest deadline remains30000.\n",
            supplied.scope
        ),
    )?;
    let config = RunConfig {
        root,
        split: "train".into(),
        python: PathBuf::from(&args[3]).canonicalize()?,
        bridge_script: PathBuf::from(&args[4]).canonicalize()?,
        output: directory.join("diagnostic-receipts.jsonl"),
        metrics: directory.join("diagnostic-metrics.jsonl"),
        trace_directory: Some(directory.join("traces")),
        limit: if cohort.is_some() { None } else { Some(limit) },
        start_index: 0,
        deadline_ms,
        partition: TrainPartition::Development,
        cohort,
    };
    config.validate_development_diagnostic()?;
    let examples = if args.len() == 17 {
        if mode != ModelMode::ActorV2 {
            return Err("training examples require full V2 experimental actor".into());
        }
        let train =
            nora_lifemate::cohort::EvaluationCohort::from_file(Path::new(&args[15]), &args[16])?;
        let index = nora_lifemate::retrieval::ExampleIndex::from_file(
            Path::new(&args[13]),
            &args[14],
            &train,
        )?;
        index.check_disjoint(
            config
                .cohort
                .as_ref()
                .ok_or("explicit evaluation cohort required")?,
        )?;
        let index = if example_before_request {
            index.with_example_before_request()
        } else {
            index
        };
        #[cfg(feature = "semantic-retrieval")]
        let index = if let Some([path, sha, assets, assets_sha]) = &semantic {
            index.with_semantic_assets(Path::new(path), sha, Path::new(assets), assets_sha)?
        } else {
            index
        };
        Some(index)
    } else {
        None
    };
    let mut model = LocalModel::start_with_assets(
        &PathBuf::from(&args[1]).canonicalize()?,
        ModelAssets {
            base: &PathBuf::from(&args[2]).canonicalize()?,
            lora: adapter.as_deref(),
        },
        context,
        "CUDA0",
        &directory.join("model.log"),
        mode,
        runtime,
    )?;
    if let Some(examples) = examples {
        model = model.with_training_examples(examples)?;
    }
    let count =
        runner::run_development_diagnostic(&config, &mut SuppliedSpeech(expected), &mut model)?;
    eprintln!("NORA text diagnostic completed {count} development samples; DO NOT SUBMIT");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_requires_the_exact_native_asset_and_preserves_other_options() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("adapter with spaces.gguf");
        // Header fixture only: native model/adapter compatibility is a separate
        // integration test on lattice, not certified by this admission test.
        let bytes = b"GGUF\x03\x00\x00\x00fixture";
        std::fs::write(&path, bytes).unwrap();
        let original = vec!["model".into(), "--cpu-output".into()];
        let mut args = original.clone();
        assert!(adapter_options(&mut args).unwrap().is_none());
        assert_eq!(args, original);
        args.extend([
            "--lora".into(),
            path.to_str().unwrap().into(),
            format!("{:x}", Sha256::digest(bytes)),
        ]);
        assert_eq!(
            adapter_options(&mut args).unwrap(),
            Some(path.canonicalize().unwrap())
        );
        assert_eq!(args, original);
        for (contents, pin) in [
            (bytes.as_slice(), "0".repeat(64)),
            (
                b"not GGUF".as_slice(),
                format!("{:x}", Sha256::digest(b"not GGUF")),
            ),
            (bytes.as_slice(), "invalid pin".into()),
        ] {
            std::fs::write(&path, contents).unwrap();
            let mut rejected = original.clone();
            rejected.extend(["--lora".into(), path.to_str().unwrap().into(), pin]);
            let before = rejected.clone();
            assert!(adapter_options(&mut rejected).is_err());
            assert_eq!(rejected, before);
        }
    }

    #[test]
    fn cpu_output_is_explicit_and_does_not_consume_cohort_or_retrieval_flags() {
        let original = (0..17).map(|i| format!("arg{i}")).collect::<Vec<_>>();
        let mut unchanged = original.clone();
        assert!(!runtime_options(&mut unchanged).unwrap().cpu_output);
        assert_eq!(unchanged, original);
        let mut args = original.clone();
        args.extend(
            [
                "--example-before-request",
                "--prefill-batch",
                "256",
                "128",
                "--gpu-layers",
                "all",
                "--cpu-output",
            ]
            .map(String::from),
        );
        let runtime = runtime_options(&mut args).unwrap();
        assert!(runtime.cpu_output);
        assert_eq!(runtime.gpu_layers, None);
        assert!(presentation_options(&mut args).unwrap());
        assert_eq!(args, original);
        let mut invalid = original;
        invalid.extend(["--gpu-layers", "257", "--cpu-output"].map(String::from));
        let before = invalid.clone();
        assert!(runtime_options(&mut invalid).is_err());
        assert_eq!(invalid, before);
    }

    #[test]
    fn explicit_placement_preserves_pins_and_checks_all_options_before_mutating() {
        let original = (0..17).map(|i| format!("arg{i}")).collect::<Vec<_>>();
        let mut args = original.clone();
        args.extend(["--prefill-batch", "256", "128", "--gpu-layers", "28"].map(String::from));
        assert_eq!(runtime_options(&mut args).unwrap().gpu_layers, Some(28));
        assert_eq!(args, original);
        for layers in ["-1", "257", "automatic"] {
            let mut args = original.clone();
            args.extend(["--gpu-layers", layers].map(String::from));
            let before = args.clone();
            assert!(runtime_options(&mut args).is_err());
            assert_eq!(args, before);
        }
        let mut args = original;
        args.extend(["--prefill-batch", "256", "512", "--gpu-layers", "28"].map(String::from));
        let before = args.clone();
        assert!(runtime_options(&mut args).is_err());
        assert_eq!(args, before);
    }

    #[test]
    fn presentation_and_semantic_options_keep_all_training_and_evaluation_pins() {
        let original = (0..17).map(|i| format!("arg{i}")).collect::<Vec<_>>();
        let mut args = original.clone();
        args.extend(
            [
                "--example-before-request",
                "--semantic-retrieval",
                "i",
                "s",
                "a",
                "s",
                "--prefill-batch",
                "256",
                "128",
            ]
            .map(String::from),
        );
        runtime_options(&mut args).unwrap();
        assert!(semantic_options(&mut args).unwrap().is_some());
        assert!(presentation_options(&mut args).unwrap());
        assert_eq!(args, original);
        let mut no_train = vec!["arg".into(); 13];
        no_train.push("--example-before-request".into());
        assert!(presentation_options(&mut no_train).is_err());
    }

    #[test]
    fn semantic_option_preserves_pins_and_requires_train_only_examples() {
        let original = (0..17).map(|i| format!("arg{i}")).collect::<Vec<_>>();
        let mut args = original.clone();
        args.extend(
            [
                "--semantic-retrieval",
                "index",
                "index_sha",
                "assets",
                "assets_sha",
                "--prefill-batch",
                "256",
                "128",
            ]
            .map(String::from),
        );
        runtime_options(&mut args).unwrap();
        assert_eq!(
            semantic_options(&mut args).unwrap().unwrap(),
            ["index", "index_sha", "assets", "assets_sha"].map(String::from)
        );
        assert_eq!(args, original);
        let mut incomplete = vec!["arg".into(); 13];
        incomplete.extend(["--semantic-retrieval", "i", "s", "a", "s"].map(String::from));
        assert!(semantic_options(&mut incomplete).is_err());
    }

    #[test]
    fn prefill_options_preserve_all_positional_inputs_and_retrieval_pins() {
        for count in [9, 12, 13, 17] {
            let original = (0..count).map(|i| format!("input_{i}")).collect::<Vec<_>>();
            let mut args = original.clone();
            runtime_options(&mut args).unwrap();
            assert_eq!(args, original);
            args.extend(["--prefill-batch".into(), "512".into(), "512".into()]);
            runtime_options(&mut args).unwrap();
            assert_eq!(args, original);
        }
    }

    #[test]
    fn invalid_prefill_options_fail_before_changing_inputs_or_starting_a_model() {
        for (batch, microbatch) in [
            ("0", "0"),
            ("512", "1024"),
            ("128", "512"),
            ("256", "33"),
            ("-1", "128"),
        ] {
            let mut args = vec![
                "original".into(),
                "--prefill-batch".into(),
                batch.into(),
                microbatch.into(),
            ];
            let before = args.clone();
            assert!(runtime_options(&mut args).is_err());
            assert_eq!(args, before);
        }
    }
}
