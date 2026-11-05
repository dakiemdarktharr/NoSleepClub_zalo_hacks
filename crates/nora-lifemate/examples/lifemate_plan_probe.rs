//! Synthetic LLM->typed-plan->executor probe, not LifeMate task accuracy.
use nora_lifemate::{
    Result,
    bridge::CallOutcome,
    local::{LocalModel, ModelMode, ModelRuntime},
    plan, planning,
    protocol::ToolSpec,
    runner::DecisionModel,
};
use serde_json::{Value, json};
use std::{
    fs::File,
    path::PathBuf,
    time::{Duration, Instant},
};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("expected LLAMA_SERVER MODEL NEW_OUTPUT_DIRECTORY".into());
    }
    let directory = PathBuf::from(&args[2]);
    std::fs::create_dir(&directory)?;
    let tools = vec![
        ToolSpec {
            name: "nora.weather".into(),
            kind: "read".into(),
            description: "Return the observed rain status.".into(),
            parameters: json!({"type":"object","properties":{},"required":[]}),
            returns: json!({"ok":"boolean","rain":"boolean"}),
        },
        ToolSpec {
            name: "nora.set".into(),
            kind: "write".into(),
            description: "Set the supplied integer value.".into(),
            parameters: json!({"type":"object","properties":{"value":{"type":"integer"}},"required":["value"]}),
            returns: json!({"ok":"boolean"}),
        },
    ];
    let mut model = LocalModel::start(
        &PathBuf::from(&args[0]).canonicalize()?,
        &PathBuf::from(&args[1]).canonicalize()?,
        6144,
        "CUDA0",
        &directory.join("model.log"),
        ModelMode::PlanTyped,
        ModelRuntime::parse(true, "all")?,
    )?;
    model.begin_sample(&tools)?;
    let messages = vec![
        json!({"role":"system","content":planning::prompt(&tools)?}),
        json!({"role":"user","content":"Create a complete data-dependent plan. First read nora.weather. If and only if its rain field is true, call nora.set with value 7. The weather result is not known yet. Do not stop at only the read; include the gated write in this plan."}),
    ];
    let response = model.complete(
        &messages,
        &planning::schema(&tools)?,
        Duration::from_secs(30),
    )?;
    serde_json::to_writer_pretty(
        File::create_new(directory.join("response.json"))?,
        &response,
    )?;
    let choice = response
        .pointer("/choices/0")
        .ok_or("missing plan choice")?;
    if choice["finish_reason"] != "stop" {
        return Err("planning probe response was truncated".into());
    }
    let text = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .ok_or("missing plan text")?;
    let generated = planning::parse_plan(text, &tools)?;
    let mut reports = Vec::new();
    for rain in [false, true] {
        let mut calls = Vec::new();
        let report = plan::execute(
            &generated,
            &tools,
            Instant::now() + Duration::from_secs(5),
            |call, _| {
                calls.push(call.clone());
                let result = match call.name.as_str() {
                    "nora.weather" => json!({"ok":true,"rain":rain}),
                    "nora.set" if call.arguments == json!({"value":7}) => json!({"ok":true}),
                    _ => return Err("synthetic plan issued wrong arguments".into()),
                };
                Ok(CallOutcome {
                    executed: true,
                    validation: json!({"ok":true}),
                    result: Some(result),
                })
            },
        );
        let expected = if rain {
            vec!["nora.weather", "nora.set"]
        } else {
            vec!["nora.weather"]
        };
        let names: Vec<_> = calls.iter().map(|c| c.name.as_str()).collect();
        if !report.completed || names != expected {
            return Err("same generated plan failed a controlled weather branch".into());
        }
        reports.push(json!({"synthetic_rain":rain,"calls":calls,"report":report}));
    }
    serde_json::to_writer_pretty(
        File::create_new(directory.join("result.json"))?,
        &json!({"scope":"synthetic LLM+typed executor branch probe; not NORA task accuracy","same_plan_passed_both_branches":true,"dataset_used":false,"sdk_used":false,"plan":generated,"reports":reports,"response":response}),
    )?;
    Ok(())
}
