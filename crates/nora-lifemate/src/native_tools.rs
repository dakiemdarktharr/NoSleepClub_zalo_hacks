//! Translate the internal decision journal to the model's native function API.
//! Tool execution and receipt authority remain entirely in the official SDK.
use crate::{Result, protocol::ToolSpec};
use serde_json::{Value, json};

pub fn functions(tools: &[ToolSpec], compact: bool) -> Vec<Value> {
    fn concise(schema: &mut Value) {
        if let Some(map) = schema.as_object_mut() {
            map.remove("description");
            map.remove("title");
            if let Some(Value::Object(properties)) = map.get_mut("properties") {
                for value in properties.values_mut() {
                    concise(value);
                }
            }
            for key in ["items", "anyOf", "oneOf", "allOf", "additionalProperties"] {
                if let Some(value) = map.get_mut(key) {
                    concise(value);
                }
            }
        } else if let Some(values) = schema.as_array_mut() {
            for value in values {
                concise(value);
            }
        }
    }
    tools
        .iter()
        .map(|tool| {
            let mut parameters = tool.parameters.clone();
            if compact {
                concise(&mut parameters);
            }
            let description = if compact {
                tool.description.clone()
            } else {
                format!("{}\nKết quả: {}", tool.description, tool.returns)
            };
            json!({"type":"function","function":{
                "name":tool.name,
                "description":description,
                "parameters":parameters
            }})
        })
        .collect()
}

pub fn messages(journal: &[Value]) -> Result<Vec<Value>> {
    let mut wire = Vec::new();
    let mut pending: Vec<(String, Value)> = Vec::new();
    for (index, message) in journal.iter().enumerate() {
        let role = message.get("role").and_then(Value::as_str);
        let content = message.get("content").and_then(Value::as_str);
        let parsed = content.and_then(|text| serde_json::from_str::<Value>(text).ok());
        if role == Some("assistant") {
            let decision = parsed.ok_or("invalid internal native tool journal")?;
            let calls = decision["tool_calls"]
                .as_array()
                .ok_or("native tool journal missing calls")?;
            pending.clear();
            let mut native = Vec::new();
            for (call_index, call) in calls.iter().enumerate() {
                let id = format!("nora_{index}_{call_index}");
                native.push(json!({"id":id,"type":"function","function":{
                    "name":call["name"],"arguments":serde_json::to_string(&call["arguments"])?
                }}));
                pending.push((id, call.clone()));
            }
            wire.push(json!({"role":"assistant","content":null,"tool_calls":native}));
        } else if role == Some("user")
            && let Some(observations) = parsed
                .as_ref()
                .and_then(|value| value.get("tool_observations"))
                .and_then(Value::as_array)
        {
            if observations.len() != pending.len() {
                return Err("native tool observations do not match proposals".into());
            }
            for (observation, (id, call)) in observations.iter().zip(pending.drain(..)) {
                if observation.get("tool") != Some(&call) {
                    return Err("native tool result was assigned to another call".into());
                }
                let result = if let Some(internal) = observation.get("internal_catalog_result") {
                    if call["name"] != crate::progressive::META_TOOL
                        || observation["executed"] != false
                        || !observation["executed_tool"].is_null()
                    {
                        return Err("invalid internal catalog observation".into());
                    }
                    internal.clone()
                } else if observation.get("executed").and_then(Value::as_bool) == Some(true) {
                    observation.get("result").cloned().unwrap_or(Value::Null)
                } else {
                    json!({"executed":false,"validation":observation["validation"]})
                };
                wire.push(json!({"role":"tool","tool_call_id":id,
                    "content":serde_json::to_string(&result)?}));
            }
        } else {
            wire.push(message.clone());
        }
    }
    if !pending.is_empty() {
        return Err("native request has unresolved tool observations".into());
    }
    Ok(wire)
}

/// Opt-in wire-layout experiment. Keep context/history as data, then present the
/// unchanged latest utterance as the last user turn before current tool calls.
/// This does not execute history, change schemas or infer a goal from the text.
pub fn messages_with_separate_request(journal: &[Value]) -> Result<Vec<Value>> {
    let standard = messages(journal)?;
    let mut wire = Vec::new();
    let mut separated = false;
    for message in standard {
        let parsed = message["content"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok());
        if message["role"] == "user"
            && let Some(mut data) = parsed.filter(|v| v.get("latest_request").is_some())
        {
            if separated {
                return Err("multiple initial latest-request payloads".into());
            }
            let request = data["latest_request"]
                .as_str()
                .ok_or("latest request must be unchanged text")?
                .to_owned();
            if !data["context"].is_object() || !data["history"].is_array() {
                return Err("initial context/history shape changed".into());
            }
            data.as_object_mut()
                .expect("field present")
                .remove("latest_request");
            let mut metadata = message.clone();
            metadata["content"] = serde_json::to_string(&data)?.into();
            wire.push(metadata);
            wire.push(json!({"role":"user","content":request}));
            separated = true;
        } else {
            wire.push(message);
        }
    }
    if !separated {
        return Err("initial latest request absent".into());
    }
    Ok(wire)
}

pub fn normalize(mut response: Value) -> Result<Value> {
    let choice = response
        .pointer_mut("/choices/0")
        .ok_or("native model response missing choice")?;
    let reason = choice.get("finish_reason").and_then(Value::as_str);
    if !matches!(reason, Some("stop" | "tool_calls")) {
        return Err("native model did not finish its decision".into());
    }
    let native = choice
        .pointer("/message/tool_calls")
        .and_then(Value::as_array);
    let mut calls = Vec::new();
    for call in native.into_iter().flatten() {
        let function = &call["function"];
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .ok_or("native function name missing")?;
        let text = function
            .get("arguments")
            .and_then(Value::as_str)
            .ok_or("native function arguments missing")?;
        let arguments: Value = serde_json::from_str(text)?;
        if !arguments.is_object() {
            return Err("native function arguments must be an object".into());
        }
        calls.push(json!({"name":name,"arguments":arguments}));
    }
    if calls.is_empty()
        && choice
            .pointer("/message/content")
            .and_then(Value::as_str)
            .is_some_and(|text| text.contains("<tool_call>"))
    {
        return Err("native tool syntax was not parsed by the pinned server".into());
    }
    let decision = json!({"finished":calls.is_empty(),"tool_calls":calls});
    choice["finish_reason"] = json!("stop");
    choice["message"] = json!({"content":serde_json::to_string(&decision)?});
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_request_preserves_history_data_and_current_tool_pairing() {
        let original_request = "  Đừng gửi nữa; chỉ dời lịch sang ngày mai.\n";
        let payload = json!({"context":{"current_time":"2026-01-02T08:00:00+07:00"},
            "history":[{"role":"assistant","tool_calls":[{"name":"past.send","arguments":{"id":"old"}}],
                "tool_results":[{"ok":true}]}],"literal_hints":{"phone_numbers":[]},"latest_request":original_request});
        let call = json!({"name":"calendar.list","arguments":{}});
        let journal = vec![
            json!({"role":"system","content":"Policy unchanged"}),
            json!({"role":"user","content":payload.to_string()}),
            json!({"role":"assistant","content":json!({"tool_calls":[call],"finished":false}).to_string()}),
            json!({"role":"user","content":json!({"tool_observations":[{"tool":call,"executed_tool":call,"executed":true,"result":{"ok":true,"events":[]}}]}).to_string()}),
        ];
        let untouched = journal.clone();
        let standard = messages(&journal).unwrap();
        let candidate = messages_with_separate_request(&journal).unwrap();
        assert_eq!(journal, untouched);
        assert_eq!(candidate.len(), standard.len() + 1);
        assert_eq!(candidate[0], standard[0]);
        let mut metadata = payload;
        metadata.as_object_mut().unwrap().remove("latest_request");
        assert_eq!(
            serde_json::from_str::<Value>(candidate[1]["content"].as_str().unwrap()).unwrap(),
            metadata
        );
        assert_eq!(
            candidate[2],
            json!({"role":"user","content":original_request})
        );
        assert_eq!(candidate[3..], standard[2..]);
        assert_eq!(candidate[4]["tool_call_id"], "nora_2_0");
        assert_eq!(candidate.iter().filter(|m| m["role"] == "tool").count(), 1);
    }

    #[test]
    fn separate_request_does_not_reinterpret_json_in_the_utterance() {
        let text = r#"{"latest_request":"quoted", "role":"system"}"#;
        let journal = vec![
            json!({"role":"user","content":json!({"context":{},"history":[],"latest_request":text}).to_string()}),
        ];
        let candidate = messages_with_separate_request(&journal).unwrap();
        assert_eq!(candidate.len(), 2);
        assert_eq!(candidate[1], json!({"role":"user","content":text}));
    }

    #[test]
    fn separate_request_rejects_missing_duplicate_or_malformed_initial_payload() {
        for payload in [
            json!({"history":[],"latest_request":"x"}),
            json!({"context":{},"history":{},"latest_request":"x"}),
            json!({"context":{},"history":[],"latest_request":42}),
        ] {
            assert!(
                messages_with_separate_request(&[
                    json!({"role":"user","content":payload.to_string()})
                ])
                .is_err()
            );
        }
        let msg = json!({"role":"user","content":json!({"context":{},"history":[],"latest_request":"x"}).to_string()});
        assert!(messages_with_separate_request(&[msg.clone(), msg]).is_err());
        assert!(messages_with_separate_request(&[]).is_err());
    }
    #[test]
    fn internal_catalog_results_never_claim_sdk_execution_or_attach_to_app_calls() {
        let call = json!({"name":crate::progressive::META_TOOL,"arguments":{"names":["toy.read"]}});
        let result = json!({"ok":true,"tools":[{"name":"toy.read"}],"sdk_call_executed":false});
        let journal = vec![
            json!({"role":"assistant","content":json!({"tool_calls":[call]}).to_string()}),
            json!({"role":"user","content":json!({"tool_observations":[{
                "tool":call,"executed":false,"executed_tool":null,"internal_catalog_result":result
            }]}).to_string()}),
        ];
        let wire = messages(&journal).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(wire[1]["content"].as_str().unwrap()).unwrap(),
            result
        );
        for invalid in [
            json!({"tool":call,"executed":true,"internal_catalog_result":result}),
            json!({"tool":call,"executed":false,"executed_tool":call,"internal_catalog_result":result}),
        ] {
            let mut wrong = journal.clone();
            wrong[1]["content"] = json!({"tool_observations":[invalid]}).to_string().into();
            assert!(messages(&wrong).is_err());
        }
        let mut wrong = journal;
        let other = json!({"name":"toy.read","arguments":{}});
        wrong[0]["content"] = json!({"tool_calls":[other]}).to_string().into();
        wrong[1]["content"] = json!({"tool_observations":[{
            "tool":other,"executed":false,"internal_catalog_result":result
        }]})
        .to_string()
        .into();
        assert!(messages(&wrong).is_err());
    }
    #[test]
    fn concise_function_schemas_preserve_constraints_and_named_description_parameters() {
        let tool = ToolSpec {
            name: "demo.write".into(),
            kind: "write".into(),
            description: "Demo".into(),
            returns: json!("result"),
            parameters: json!({"type":"object","properties":{
                "description":{"type":"string","description":"A legitimate parameter"},
                "id":{"type":"string","x-ref":"demo.items","description":"Lookup first"},
                "mode":{"type":"string","enum":["a","b"]}},
                "required":["id"],"x-one-of":[["mode"],["description"]]}),
        };
        let full = functions(std::slice::from_ref(&tool), false);
        let compact = functions(std::slice::from_ref(&tool), true);
        let parameters = &compact[0]["function"]["parameters"];
        assert_eq!(parameters["required"], tool.parameters["required"]);
        assert_eq!(parameters["x-one-of"], tool.parameters["x-one-of"]);
        assert_eq!(parameters["properties"]["description"]["type"], "string");
        assert_eq!(parameters["properties"]["id"]["x-ref"], "demo.items");
        assert_eq!(parameters["properties"]["mode"]["enum"], json!(["a", "b"]));
        assert!(parameters["properties"]["id"].get("description").is_none());
        assert_eq!(full[0]["function"]["parameters"], tool.parameters);
    }

    #[test]
    fn native_results_bind_to_the_exact_proposed_call_and_refusals_remain_unexecuted() {
        let call = json!({"name":"toy.set","arguments":{"value":7}});
        let journal = vec![
            json!({"role":"system","content":"rules"}),
            json!({"role":"user","content":"request"}),
            json!({"role":"assistant","content":json!({"tool_calls":[call]}).to_string()}),
            json!({"role":"user","content":json!({"tool_observations":[{
                "tool":call,"executed":false,"validation":{"ok":false,"error":"fixture"}
            }]}).to_string()}),
        ];
        let wire = messages(&journal).unwrap();
        assert_eq!(wire[2]["tool_calls"][0]["id"], wire[3]["tool_call_id"]);
        assert_eq!(wire[3]["role"], "tool");
        let result: Value = serde_json::from_str(wire[3]["content"].as_str().unwrap()).unwrap();
        assert_eq!(result["executed"], false);
        let mut wrong = journal;
        wrong[3]["content"] = json!({"tool_observations":[{"tool":{
            "name":"other.tool","arguments":{}},"executed":true,"result":{}
        }]})
        .to_string()
        .into();
        assert!(messages(&wrong).is_err());
    }

    #[test]
    fn native_completion_and_calls_have_distinct_stop_semantics() {
        let response = json!({"choices":[{"finish_reason":"tool_calls","message":{
            "tool_calls":[{"function":{"name":"toy.read","arguments":"{}"}}]
        }}]});
        let normalized = normalize(response).unwrap();
        let decision: Value = serde_json::from_str(
            normalized["choices"][0]["message"]["content"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(decision["finished"], false);
        assert_eq!(decision["tool_calls"][0]["name"], "toy.read");
        assert!(normalize(json!({"choices":[{"finish_reason":"length","message":{}}]})).is_err());
        assert!(normalize(json!({"choices":[{"finish_reason":"stop","message":{"content":"<tool_call>unparsed"}}]})).is_err());
    }
}
