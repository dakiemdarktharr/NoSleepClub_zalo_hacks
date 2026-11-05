//! Train-only equivalent target conversion. No model, inference or SDK actions.
use nora_lifemate::{
    MAX_JSON_BYTES, MAX_MODEL_TURNS, Result,
    actor_v2::Evidence,
    bridge::CallOutcome,
    cohort::EvaluationCohort,
    protocol::{Catalog, HistoryTurn, ToolCall},
    training_bindings::bind_observed_ids,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
};

fn sha(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 8 {
        return Err(
            "expected INPUT INPUT_SHA CATALOG CATALOG_SHA IDS IDS_SHA OUTPUT REPORT".into(),
        );
    }
    let input = Path::new(&args[0]);
    let catalog_path = Path::new(&args[2]);
    let output = Path::new(&args[6]);
    let report = Path::new(&args[7]);
    if output.exists()
        || report.exists()
        || output == report
        || sha(input)? != args[1]
        || sha(catalog_path)? != args[3]
    {
        return Err("source hash mismatch or occupied output/report".into());
    }
    let allowed = EvaluationCohort::from_file(Path::new(&args[4]), &args[5])?;
    let catalog: Catalog = serde_json::from_reader(File::open(catalog_path)?)?;
    let tools = catalog
        .apps
        .into_iter()
        .flat_map(|app| app.tools)
        .collect::<Vec<_>>();
    let mut staged = tempfile::NamedTempFile::new_in(output.parent().ok_or("output parent")?)?;
    let mut seen = BTreeSet::new();
    let mut sample = String::new();
    let mut old_targets = Vec::<Value>::new();
    let mut old_texts = Vec::<String>::new();
    let mut new_targets = Vec::<Value>::new();
    let mut new_texts = Vec::<String>::new();
    let mut records = 0;
    let mut bound_arguments = 0;
    let mut bound_records = 0;
    for line in BufReader::new(File::open(input)?).lines() {
        let line = line?;
        if line.len() > MAX_JSON_BYTES {
            return Err("training record exceeds byte bound".into());
        }
        let mut row: Value = serde_json::from_str(&line)?;
        let sid = row["sample_id"]
            .as_str()
            .ok_or("sample ID absent")?
            .to_owned();
        if row["partition"] != "train" || !allowed.contains(&sid) {
            return Err("internal training whitelist only".into());
        }
        if sid != sample {
            if !seen.insert(sid.clone()) {
                return Err("noncontiguous or duplicated sample".into());
            }
            sample = sid;
            old_targets.clear();
            old_texts.clear();
            new_targets.clear();
            new_texts.clear();
        }
        let turn = old_targets.len();
        if turn >= MAX_MODEL_TURNS || row["example_id"] != format!("{sample}:actor:{turn:03}") {
            return Err("missing or reordered training turn".into());
        }
        let old_target = row["target"].clone();
        let old_text = row["target_text"]
            .as_str()
            .ok_or("target text absent")?
            .to_owned();
        if serde_json::from_str::<Value>(&old_text)? != old_target {
            return Err("target and target_text differ".into());
        }
        let mut assistant_index = 0;
        let mut observation_index = 0;
        let mut observations = Vec::new();
        let mut evidence = None;
        for message in row["model_input"]["messages"]
            .as_array_mut()
            .ok_or("messages absent")?
        {
            if message["role"] == "assistant" {
                if old_texts
                    .get(assistant_index)
                    .is_none_or(|t| message["content"] != *t)
                {
                    return Err("past assistant target differs from previous causal record".into());
                }
                message["content"] = json!(new_texts[assistant_index]);
                assistant_index += 1;
                continue;
            }
            if message["role"] != "user" {
                continue;
            }
            let mut body: Value =
                serde_json::from_str(message["content"].as_str().ok_or("user text absent")?)?;
            if body.get("latest_request").is_some() {
                if evidence.is_some() {
                    return Err("multiple initial contexts".into());
                }
                let history: Vec<HistoryTurn> = serde_json::from_value(body["history"].clone())?;
                evidence = Some(Evidence::begin(&body["context"], &history, &tools));
            }
            if let Some(items) = body.get_mut("tool_observations") {
                for item in items
                    .as_array_mut()
                    .ok_or("observations must be an array")?
                {
                    let previous = old_targets
                        .get(observation_index)
                        .ok_or("future observation")?;
                    let calls = previous["tool_calls"]
                        .as_array()
                        .ok_or("past calls missing")?;
                    if calls.len() != 1
                        || item["tool"] != calls[0]
                        || item["executed_tool"] != calls[0]
                        || item["executed"] != true
                    {
                        return Err(
                            "requires the original one-call, executed gold trajectories".into()
                        );
                    }
                    let call: ToolCall = serde_json::from_value(calls[0].clone())?;
                    let outcome = CallOutcome {
                        executed: true,
                        validation: item["validation"].clone(),
                        result: Some(item["result"].clone()),
                    };
                    let progress = evidence
                        .as_mut()
                        .ok_or("initial context missing")?
                        .record(&call, &outcome, &tools);
                    if progress != item["progress"] {
                        return Err("runtime evidence feedback differs".into());
                    }
                    observations.push(json!({"call":call,"executed":true,"result":item["result"]}));
                    item["tool"] = new_targets[observation_index]["tool_calls"][0].clone();
                    observation_index += 1;
                }
            }
            message["content"] = serde_json::to_string(&body)?.into();
        }
        if assistant_index != turn || observation_index != turn {
            return Err("incomplete causal prefix".into());
        }
        let evidence = evidence.ok_or("initial context absent")?;
        let mut target = old_target.clone();
        let before = bound_arguments;
        for call in target["tool_calls"]
            .as_array_mut()
            .ok_or("target calls missing")?
        {
            let labelled: ToolCall = serde_json::from_value(call.clone())?;
            let proposed = bind_observed_ids(&labelled, &observations, &tools);
            if evidence.bind(&proposed)? != labelled {
                return Err("binding changed a labelled SDK argument".into());
            }
            bound_arguments += proposed
                .arguments
                .as_object()
                .ok_or("arguments object required")?
                .iter()
                .filter(|(key, value)| labelled.arguments[*key] != **value)
                .count();
            *call = serde_json::to_value(proposed)?;
        }
        bound_records += usize::from(bound_arguments != before);
        let target_text = serde_json::to_string(&target)?;
        row["target"] = target.clone();
        row["target_text"] = target_text.clone().into();
        row["metadata"]["target_representation"] = json!("unique_observed_id_bindings_v1");
        serde_json::to_writer(staged.as_file_mut(), &row)?;
        staged.write_all(b"\n")?;
        old_targets.push(old_target);
        old_texts.push(old_text);
        new_targets.push(target);
        new_texts.push(target_text);
        records += 1;
    }
    if seen.len() != allowed.len() || sha(input)? != args[1] || sha(catalog_path)? != args[3] {
        return Err("incomplete coverage or changed source".into());
    }
    let receipt = json!({"scope":"training-only equivalent ID target representation; no model or SDK execution",
        "status":"complete","samples":seen.len(),"records":records,"bound_arguments":bound_arguments,
        "bound_records":bound_records,"input_sha256":args[1],"catalog_sha256":args[3],"train_ids_sha256":args[5],
        "output_sha256":sha(staged.path())?,"all_targets_resolve_to_original_calls":true,
        "full_runtime_prefix_replay_passed":false,"quality_evaluated":false});
    let mut receipt_file =
        tempfile::NamedTempFile::new_in(report.parent().ok_or("report parent")?)?;
    serde_json::to_writer_pretty(receipt_file.as_file_mut(), &receipt)?;
    receipt_file.write_all(b"\n")?;
    staged.persist_noclobber(output)?;
    receipt_file.persist_noclobber(report)?;
    println!("{receipt}");
    Ok(())
}
