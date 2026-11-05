//! TRAIN-only supervised prefix parity. No model, accuracy, voice or submission.
use nora_lifemate::{
    Result,
    cohort::EvaluationCohort,
    protocol::ToolSpec,
    runner::{self, DecisionModel, RunConfig, Speech, TrainPartition},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Input {
    messages: Vec<Value>,
}
#[derive(Deserialize)]
struct Record {
    example_id: String,
    sample_id: String,
    partition: String,
    model_input: Input,
    target_text: String,
}
struct Replay {
    groups: VecDeque<VecDeque<Record>>,
    current: VecDeque<Record>,
    matched: usize,
    errors: File,
}
impl DecisionModel for Replay {
    fn actor_v2(&self) -> bool {
        true
    }
    fn compact_prompt(&self) -> bool {
        true
    }
    fn begin_sample(&mut self, _tools: &[ToolSpec]) -> Result<()> {
        if !self.current.is_empty() {
            writeln!(
                self.errors,
                "{}",
                json!({"unconsumed":self.current.iter().map(|r|&r.example_id).collect::<Vec<_>>()})
            )?;
        }
        self.current = self
            .groups
            .pop_front()
            .ok_or("supervision sample exhausted")?;
        Ok(())
    }
    fn complete(
        &mut self,
        messages: &[Value],
        _schema: &Value,
        _remaining: Duration,
    ) -> Result<Value> {
        let record = self
            .current
            .pop_front()
            .ok_or("runtime requested an unlabelled recovery turn")?;
        if messages != record.model_input.messages {
            let mismatch = messages
                .iter()
                .zip(&record.model_input.messages)
                .position(|(a, b)| a != b);
            writeln!(
                self.errors,
                "{}",
                json!({"example_id":record.example_id,"expected_messages":record.model_input.messages.len(),"actual_messages":messages.len(),"first_mismatch":mismatch,"actual":mismatch.map(|i|&messages[i]),"expected":mismatch.map(|i|&record.model_input.messages[i])})
            )?;
            return Err(
                "supervised prefix differs from real runtime; quarantine before training".into(),
            );
        }
        self.matched += 1;
        Ok(json!({"choices":[{"finish_reason":"stop","message":{"content":record.target_text}}]}))
    }
}
struct References(BTreeMap<PathBuf, String>);
impl Speech for References {
    fn transcribe(&mut self, p: &Path, _deadline: Instant) -> Result<String> {
        self.0
            .get(p)
            .cloned()
            .ok_or_else(|| "training reference absent".into())
    }
}
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.len() == 3 && a[0] == "--canonicalize-user-messages" {
        let mut output = std::io::BufWriter::new(File::create_new(&a[2])?);
        let mut count = 0;
        for line in BufReader::new(File::open(&a[1])?).lines() {
            let mut row: Value = serde_json::from_str(&line?)?;
            if row["partition"] != "train" {
                return Err("training records only".into());
            }
            for msg in row["model_input"]["messages"]
                .as_array_mut()
                .ok_or("messages absent")?
            {
                if msg["role"] != "user" {
                    continue;
                }
                let mut content: Value =
                    serde_json::from_str(msg["content"].as_str().ok_or("text absent")?)?;
                if let Some(text) = content["latest_request"].as_str() {
                    let phones = nora_lifemate::grounding::spoken_phone_numbers(text);
                    content["literal_hints"]["phone_numbers"] = json!(phones);
                }
                // Use exactly the runtime's float/string JSON serialization.
                // Targets, history, observations and chosen arguments are not edited.
                msg["content"] = serde_json::to_string(&content)?.into();
            }
            serde_json::to_writer(&mut output, &row)?;
            output.write_all(b"\n")?;
            count += 1;
        }
        output.flush()?;
        output.get_ref().sync_all()?;
        println!("Canonicalized {count} training records; no model or score");
        return Ok(());
    }
    if a.len() != 8 {
        return Err(
            "expected ROOT PYTHON BRIDGE COHORT COHORT_SHA TRAIN_RECORDS REFERENCES NEW_DIR".into(),
        );
    }
    let root = PathBuf::from(&a[0]).canonicalize()?;
    let cohort = EvaluationCohort::from_file(Path::new(&a[3]), &a[4])?;
    let mut groups: VecDeque<VecDeque<Record>> = VecDeque::new();
    let mut expected = 0;
    for line in BufReader::new(File::open(&a[5])?).lines() {
        let r: Record = serde_json::from_str(&line?)?;
        if r.partition != "train" || !cohort.contains(&r.sample_id) {
            return Err("internal-train supervision only".into());
        }
        if groups
            .back()
            .and_then(|g| g.front())
            .is_none_or(|last| last.sample_id != r.sample_id)
        {
            groups.push_back(VecDeque::new());
        }
        groups.back_mut().expect("group inserted").push_back(r);
        expected += 1;
    }
    if groups.len() != cohort.len() {
        return Err("supervision/cohort coverage mismatch".into());
    }
    let refs: Value = serde_json::from_reader(File::open(&a[6])?)?;
    if refs["scope"] != "internal-train-supervision-reference-only" {
        return Err("reference scope differs".into());
    }
    let mut references = BTreeMap::new();
    for (name, text) in refs["by_audio"].as_object().ok_or("references missing")? {
        references.insert(
            nora_lifemate::protocol::audio_path(&root.join("train"), name)?,
            text.as_str().ok_or("reference not text")?.to_owned(),
        );
    }
    let out = PathBuf::from(&a[7]);
    std::fs::create_dir(&out)?;
    std::fs::write(
        out.join("DIAGNOSTIC_ONLY.txt"),
        "SUPERVISED GOLD REPLAY. No model inference. NOT official results. NOT accuracy. Do not submit.",
    )?;
    let config = RunConfig {
        root,
        split: "train".into(),
        python: PathBuf::from(&a[1]).canonicalize()?,
        bridge_script: PathBuf::from(&a[2]).canonicalize()?,
        output: out.join("diagnostic-receipts.jsonl"),
        metrics: out.join("diagnostic-metrics.jsonl"),
        trace_directory: None,
        limit: None,
        start_index: 0,
        deadline_ms: 30_000,
        partition: TrainPartition::Development,
        cohort: Some(cohort),
    };
    let mut model = Replay {
        groups,
        current: VecDeque::new(),
        matched: 0,
        errors: File::create_new(out.join("prefix-differences.jsonl"))?,
    };
    let count =
        runner::run_development_diagnostic(&config, &mut References(references), &mut model)?;
    let report = json!({"scope":"supervised internal-train runtime byte parity, NOT model accuracy","samples":count,"expected_prefixes":expected,"matched_prefixes":model.matched,"unstarted_groups":model.groups.len(),"last_unconsumed":model.current.len(),"model_inference":false,"official_score":null});
    model.errors.sync_all()?;
    std::fs::write(
        out.join("parity-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
