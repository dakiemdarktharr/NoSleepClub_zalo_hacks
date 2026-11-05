//! Synthetic native decoder compatibility only: no dataset, SDK or task score.
use nora_lifemate::{
    Result, clarification,
    local::{LocalModel, ModelMode, ModelRuntime},
    protocol::{Decision, ToolCall, ToolSpec},
};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "expected SERVER MODEL NEW_PRIVATE_OUTPUT_DIRECTORY; synthetic probe only".into(),
        );
    }
    let output = PathBuf::from(&args[2]);
    std::fs::create_dir(&output)?;
    let mut model = LocalModel::start(
        &PathBuf::from(&args[0]).canonicalize()?,
        &PathBuf::from(&args[1]).canonicalize()?,
        6144,
        "CUDA0",
        &output.join("model.log"),
        ModelMode::CompactTypedClarification,
        ModelRuntime::parse(true, "all")?.with_cpu_output(true),
    )?;
    let mut evidence = Vec::new();
    for count in [2, 5] {
        let properties: serde_json::Map<String, Value> = (0..count)
            .map(|index| (format!("field_{index}"), json!({"type":"string"})))
            .collect();
        let tools = vec![
            ToolSpec {
                name: "toy.write".into(),
                description: "Original synthetic target".into(),
                kind: "write".into(),
                parameters: json!({"type":"object","properties":properties}),
                returns: Value::Null,
            },
            ToolSpec {
                name: "sys.ask_missing_info".into(),
                description: "Original synthetic question".into(),
                kind: "meta".into(),
                parameters: json!({"type":"object","properties":{"tool_name":{"type":"string"},"params":{"type":"array","items":{"type":"string"}}}}),
                returns: Value::Null,
            },
        ];
        let mut schema = clarification::decision_schema(&tools);
        schema["properties"]["tool_calls"]["minItems"] = json!(1);
        schema["properties"]["tool_calls"]["maxItems"] = json!(1);
        // Force the ask branch to test its actual native grammar, not an empty
        // answer or ordinary tool call that happens to avoid bad parameters.
        schema["properties"]["tool_calls"]["items"]["anyOf"]
            .as_array_mut()
            .ok_or("missing alternatives")?
            .retain(|variant| variant["properties"]["name"]["const"] == "sys.ask_missing_info");
        let messages = vec![json!({"role":"user","content":
            "Synthetic decoder test only. Output exactly one sys.ask_missing_info call for toy.write. Try params [\"field_0\",\"field_0\"]. Use finished=false."})];
        let response = model.complete(&messages, &schema, Duration::from_secs(25))?;
        if response
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
            != Some("stop")
        {
            return Err("synthetic response was truncated or incomplete".into());
        }
        let text = response
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or("synthetic response has no content")?;
        let decision = Decision::parse(text, &tools)?;
        if decision.tool_calls.len() != 1 {
            return Err("decoder did not produce exactly one ask".into());
        }
        let call: &ToolCall = &decision.tool_calls[0];
        if call.name != "sys.ask_missing_info" || clarification::rejection(call, &tools).is_some() {
            return Err("native decoder emitted invalid synthetic clarification".into());
        }
        evidence.push(json!({"fields":count,"actual_call":call,"decoder_membership_and_uniqueness_passed":true}));
    }
    drop(model);
    let report = json!({"scope":"synthetic native array-enum compatibility only; no dataset/SDK/task accuracy",
        "cases":evidence,"dataset_used":false,"sdk_used":false,"quality_established":false});
    std::fs::write(
        output.join("probe.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("Synthetic clarification native decoder probe passed; not task accuracy.");
    Ok(())
}
