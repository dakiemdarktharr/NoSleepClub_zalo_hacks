//! Synthetic format compatibility only. No dataset or SDK is opened.
use nora_lifemate::{
    Result,
    local::{LocalModel, ModelMode, ModelRuntime},
    protocol::{Decision, ToolSpec, decision_schema},
    runner::DecisionModel,
};
use serde_json::json;
use std::{fs::File, path::PathBuf, time::Duration};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("expected LLAMA_SERVER XLAM_MODEL NEW_OUTPUT_DIRECTORY".into());
    }
    let directory = PathBuf::from(&args[2]);
    std::fs::create_dir(&directory)?;
    let tool = ToolSpec {
        name: "nora.echo".into(),
        kind: "read".into(),
        description: "Echo the supplied message; use once when requested.".into(),
        parameters: json!({"type":"object","properties":{"message":{"type":"string"}},"required":["message"],"additionalProperties":false}),
        returns: json!({"ok":"boolean","message":"string"}),
    };
    let mut model = LocalModel::start(
        &PathBuf::from(&args[0]).canonicalize()?,
        &PathBuf::from(&args[1]).canonicalize()?,
        6144,
        "CUDA0",
        &directory.join("model.log"),
        ModelMode::XlamNative,
        ModelRuntime::parse(true, "all")?,
    )?;
    let tools = vec![tool];
    model.begin_sample(&tools)?;
    let mut messages = vec![
        json!({"role":"system","content":format!("You are a tool-using assistant. Use only supplied tools; never invent results. {}",nora_lifemate::xlam::FORMAT_INSTRUCTIONS)}),
        json!({"role":"user","content":"Call nora.echo exactly once with message nora-probe. After receiving its result, say done."}),
    ];
    let first = model.complete(&messages, &decision_schema(&tools), Duration::from_secs(30))?;
    let text = first
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .ok_or("missing first decision")?;
    let decision = Decision::parse(text, &tools)?;
    if decision.tool_calls.len() != 1
        || decision.tool_calls[0].name != "nora.echo"
        || decision.tool_calls[0].arguments != json!({"message":"nora-probe"})
    {
        return Err("synthetic call contract mismatch".into());
    }
    messages.push(json!({"role":"assistant","content":text}));
    messages.push(json!({"role":"user","content":json!({"tool_observations":[{"tool":decision.tool_calls[0],"validation":{"ok":true},"executed":true,"result":{"ok":true,"message":"nora-probe"}}]}).to_string()}));
    let second = model.complete(&messages, &decision_schema(&tools), Duration::from_secs(30))?;
    let text = second
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .ok_or("missing completion decision")?;
    let end = Decision::parse(text, &tools)?;
    if !end.finished || !end.tool_calls.is_empty() {
        return Err("synthetic completion contract mismatch".into());
    }
    serde_json::to_writer_pretty(
        File::create_new(directory.join("result.json"))?,
        &json!({"scope":"synthetic format compatibility; not NORA task accuracy","call_then_result_then_completion":true,"dataset_used":false,"sdk_used":false,"first":first,"second":second}),
    )?;
    Ok(())
}
