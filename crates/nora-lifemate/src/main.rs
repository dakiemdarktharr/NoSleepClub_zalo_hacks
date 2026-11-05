use nora_lifemate::Result;

fn main() {
    if let Err(error) = run() {
        eprintln!("NORA LifeMate: {error}");
        std::process::exit(2);
    }
}

#[cfg(not(feature = "whisper"))]
fn run() -> Result<()> {
    Err("build nora-lifemate with --features whisper to enable offline inference".into())
}

#[cfg(feature = "whisper")]
fn run() -> Result<()> {
    use nora_lifemate::{
        local::{LocalModel, ModelAssets, ModelMode, ModelRuntime},
        runner::{self, RunConfig, TrainPartition},
        speech::{self, SpeechWorker},
    };
    use std::{collections::BTreeMap, path::PathBuf, time::Instant};
    let mut args = std::env::args().skip(1);
    if std::env::args().nth(1).as_deref() == Some("--speech-worker") {
        let arguments: Vec<String> = args.collect();
        if arguments.len() != 3 {
            return Err("invalid speech worker arguments".into());
        }
        return speech::worker_main(&PathBuf::from(&arguments[1]), &PathBuf::from(&arguments[2]));
    }
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        if key == "--help" {
            println!(
                "nora-lifemate --root DATA_ROOT --split train|public_test|private_test --python PYTHON --bridge BRIDGE.py --server LLAMA_SERVER --model MODEL.gguf --device Vulkan1|CUDA0 --asr-model WHISPER.bin --output NEW.jsonl --metrics NEW.jsonl --model-log NEW.log [--asr-server WHISPER_SERVER] [--asr-beam 1..10] [--context 8192] [--limit N] [--start-index N] [--trace NEW_TRAIN_DIR] [--deadline-ms 30000] [--partition development|holdout|all] [--llm-mode legacy|compact|compact-typed|compact-typed-alias-experimental|compact-typed-fewshot-experimental|typed|native|native-greedy|native-compact|xlam-native|xlam-progressive|native-thinking|plan-typed-experimental] [--prompt-cache on|off] [--gpu-layers all|0..256] [--cpu-output on|off]"
            );
            println!(
                "Experimental native tool disclosure: --tool-catalog full|progressive (default full)."
            );
            println!(
                "Train evaluation only: --cohort-ids IDS.json --cohort-sha256 HEX (no start/limit; preserves dataset order)."
            );
            println!(
                "Explicit adapter: --lora ADAPTER.gguf --lora-sha256 HEX. Experimental actor: --llm-mode actor-v2-experimental."
            );
            println!(
                "Prefill override: --prefill-batch 32..512 --prefill-microbatch 32..512 (powers of two, microbatch <= batch)."
            );
            return Ok(());
        }
        if !matches!(
            key.as_str(),
            "--root"
                | "--split"
                | "--python"
                | "--bridge"
                | "--server"
                | "--model"
                | "--device"
                | "--asr-model"
                | "--asr-server"
                | "--asr-beam"
                | "--output"
                | "--metrics"
                | "--model-log"
                | "--context"
                | "--limit"
                | "--start-index"
                | "--trace"
                | "--deadline-ms"
                | "--partition"
                | "--llm-mode"
                | "--prompt-cache"
                | "--gpu-layers"
                | "--cpu-output"
                | "--tool-catalog"
                | "--cohort-ids"
                | "--cohort-sha256"
                | "--lora"
                | "--lora-sha256"
                | "--prefill-batch"
                | "--prefill-microbatch"
        ) {
            return Err("unsupported contest option".into());
        }
        let value = args.next().ok_or("missing contest option value")?;
        if values.insert(key, value).is_some() {
            return Err("duplicate contest option".into());
        }
    }
    let required = |name: &str| {
        values
            .get(name)
            .cloned()
            .ok_or_else(|| format!("missing {name}"))
    };
    let config = RunConfig {
        root: PathBuf::from(required("--root")?).canonicalize()?,
        split: required("--split")?,
        python: PathBuf::from(required("--python")?).canonicalize()?,
        bridge_script: PathBuf::from(required("--bridge")?).canonicalize()?,
        output: PathBuf::from(required("--output")?),
        metrics: PathBuf::from(required("--metrics")?),
        trace_directory: values.get("--trace").map(PathBuf::from),
        limit: values.get("--limit").map(|s| s.parse()).transpose()?,
        start_index: values
            .get("--start-index")
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(0),
        deadline_ms: values
            .get("--deadline-ms")
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(30_000),
        partition: TrainPartition::parse(values.get("--partition").map(String::as_str).unwrap_or(
            if required("--split")? == "train" {
                "development"
            } else {
                "all"
            },
        ))?,
        cohort: match (values.get("--cohort-ids"), values.get("--cohort-sha256")) {
            (Some(path), Some(pin)) => Some(nora_lifemate::cohort::EvaluationCohort::from_file(
                &PathBuf::from(path),
                pin,
            )?),
            (None, None) => None,
            _ => return Err("cohort IDs and SHA256 must be supplied together".into()),
        },
    };
    config.validate()?;
    let asr_beam: u32 = values
        .get("--asr-beam")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(1);
    if !(1..=10).contains(&asr_beam)
        || values.contains_key("--asr-beam") && !values.contains_key("--asr-server")
    {
        return Err("ASR beam must be 1..10 and requires --asr-server when specified".into());
    }
    let model_mode = ModelMode::parse(
        values
            .get("--llm-mode")
            .map(String::as_str)
            .unwrap_or("legacy"),
    )?;
    let prompt_cache = match values
        .get("--prompt-cache")
        .map(String::as_str)
        .unwrap_or("on")
    {
        "on" => true,
        "off" => false,
        _ => return Err("invalid prompt cache setting".into()),
    };
    let cpu_output = match values
        .get("--cpu-output")
        .map(String::as_str)
        .unwrap_or("off")
    {
        "on" => true,
        "off" => false,
        _ => return Err("invalid CPU output setting".into()),
    };
    let mut model_runtime = ModelRuntime::parse(
        prompt_cache,
        values
            .get("--gpu-layers")
            .map(String::as_str)
            .unwrap_or("all"),
    )?
    .with_cpu_output(cpu_output)
    .with_catalog_disclosure(
        match values
            .get("--tool-catalog")
            .map(String::as_str)
            .unwrap_or("full")
        {
            "full" => false,
            "progressive" => true,
            _ => return Err("invalid tool catalog mode; expected full or progressive".into()),
        },
    );
    match (
        values.get("--prefill-batch"),
        values.get("--prefill-microbatch"),
    ) {
        (Some(batch), Some(microbatch)) => {
            model_runtime =
                model_runtime.with_prefill_batch(batch.parse()?, microbatch.parse()?)?;
        }
        (None, None) => {}
        _ => return Err("prefill batch and microbatch must be supplied together".into()),
    }
    let adapter = match (values.get("--lora"), values.get("--lora-sha256")) {
        (Some(path), Some(pin)) => Some(nora_lifemate::assets::pinned_lora(
            &PathBuf::from(path),
            pin,
        )?),
        (None, None) => None,
        _ => return Err("LoRA adapter and SHA256 must be supplied together".into()),
    };
    #[cfg(windows)]
    let guard = nora_platform::BudgetJob::install_contest()?;
    #[cfg(windows)]
    let _measurement = nora_platform::Measurement::start(
        guard.clone(),
        &config.metrics.with_extension("memory.json"),
    )?;
    let startup = Instant::now();
    let mut model = LocalModel::start_with_assets(
        &PathBuf::from(required("--server")?).canonicalize()?,
        ModelAssets {
            base: &PathBuf::from(required("--model")?).canonicalize()?,
            lora: adapter.as_deref(),
        },
        values
            .get("--context")
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(8192),
        &required("--device")?,
        &PathBuf::from(required("--model-log")?),
        model_mode,
        model_runtime,
    )?;
    let asr_model = PathBuf::from(required("--asr-model")?).canonicalize()?;
    let mut speech: Box<dyn runner::Speech> = if let Some(server) = values.get("--asr-server") {
        Box::new(nora_lifemate::speech_gpu::GpuSpeech::start_with_beam(
            &PathBuf::from(server).canonicalize()?,
            &asr_model,
            &config.metrics.with_extension("asr.log"),
            asr_beam,
        )?)
    } else {
        Box::new(SpeechWorker::start(
            &std::env::current_exe()?,
            &asr_model,
            &config.root.join(&config.split).canonicalize()?,
        )?)
    };
    eprintln!(
        "NORA offline models ready after {:.0} ms; local model PID {}",
        startup.elapsed().as_secs_f64() * 1000.0,
        model.pid()
    );
    let count = runner::run(&config, speech.as_mut(), &mut model)?;
    eprintln!("NORA completed {count} samples; results remain local until explicitly submitted.");
    Ok(())
}
