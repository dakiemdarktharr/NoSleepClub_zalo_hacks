//! Experimental xLAM response adapter. It preserves the model's trained bare
//! array format without guessing, repairing, or executing any proposed calls.
//! Actual tool validation and receipt authority remain in the unchanged SDK.

use crate::{
    MAX_TOOL_CALLS, Result,
    protocol::{ToolCall, ToolSpec},
};
use serde_json::{Value, json};

/// xLAM's template omits its default format guidance when a custom system
/// message is supplied. Add this guidance to NORA's task/permission rules.
pub const FORMAT_INSTRUCTIONS: &str = r#"For tool use, emit a single JSON array such as [{"name":"app.tool","arguments":{}}]. Each item has only name and arguments; arguments is a JSON object. Put independent calls in the same array and wait for their results before planning dependent calls. Do not add a tool_calls/finished envelope or Markdown fences. Follow the task's clarification-tool policy. When no further tool call is needed, provide a brief plain-text completion or an empty array."#;

const MAX_CONTENT_BYTES: usize = 65_536;

fn validate_calls(calls: &[ToolCall], tools: &[ToolSpec]) -> Result<()> {
    if calls.len() > MAX_TOOL_CALLS {
        return Err("xLAM response exceeds the tool-call budget".into());
    }
    if calls
        .iter()
        .any(|call| !call.arguments.is_object() || !tools.iter().any(|tool| tool.name == call.name))
    {
        return Err("xLAM response contains unavailable or invalid tool calls".into());
    }
    Ok(())
}

fn validate_normalized(response: &Value, tools: &[ToolSpec]) -> Result<()> {
    let content = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or("xLAM normalized response is missing its decision")?;
    // Deserialize only our normalized decision envelope. The model-facing bare
    // array has already been parsed strictly before reaching this function.
    let decision: crate::protocol::Decision = serde_json::from_str(content)?;
    validate_calls(&decision.tool_calls, tools)
}

/// Prefer calls already parsed by the native server. A bare-array fallback is
/// strict and consumes the entire response; malformed arrays cannot silently
/// turn into a successful empty decision. This function performs no inference.
pub fn normalize(mut response: Value, tools: &[ToolSpec]) -> Result<Value> {
    let choice = response
        .pointer("/choices/0")
        .ok_or("xLAM model response is missing its choice")?;
    if !matches!(
        choice.get("finish_reason").and_then(Value::as_str),
        Some("stop" | "tool_calls")
    ) {
        return Err("xLAM model did not finish its response".into());
    }
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or("xLAM model response is missing its message")?;
    let has_native_calls = match message.get("tool_calls") {
        Some(Value::Array(calls)) => !calls.is_empty(),
        None | Some(Value::Null) => false,
        Some(_) => return Err("xLAM native tool calls must be an array".into()),
    };
    if has_native_calls {
        let normalized = crate::native_tools::normalize(response)?;
        validate_normalized(&normalized, tools)?;
        return Ok(normalized);
    }

    let content = message
        .get("content")
        .and_then(Value::as_str)
        .ok_or("xLAM response without native calls must contain text")?;
    if content.len() > MAX_CONTENT_BYTES {
        return Err("xLAM response exceeds the output bound".into());
    }
    let content = content.trim();
    if content.starts_with('[') {
        // ToolCall denies unknown envelope keys. serde_json rejects trailing
        // prose, truncated JSON, non-arrays and invalid item types.
        let calls: Vec<ToolCall> = serde_json::from_str(content)?;
        validate_calls(&calls, tools)?;
        let decision = json!({"finished":calls.is_empty(),"tool_calls":calls});
        let choice = response
            .pointer_mut("/choices/0")
            .ok_or("xLAM model response is missing its choice")?;
        choice["finish_reason"] = json!("stop");
        choice["message"] = json!({"content":serde_json::to_string(&decision)?});
        return Ok(response);
    }

    // Ordinary text is the documented completion path. Retain the native
    // normalizer's refusal of unparsed tagged tool syntax as well.
    let normalized = crate::native_tools::normalize(response)?;
    validate_normalized(&normalized, tools)?;
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<ToolSpec> {
        ["demo.read", "demo.write"]
            .into_iter()
            .map(|name| ToolSpec {
                name: name.into(),
                kind: "read".into(),
                description: "Fixture tool".into(),
                parameters: json!({"type":"object"}),
                returns: Value::Null,
            })
            .collect()
    }

    fn response(content: &str) -> Value {
        json!({"id":"fixture-response","model":"fixture-xlam","usage":{"total_tokens":42},
            "choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":content}}]})
    }

    fn decision(response: &Value) -> Value {
        serde_json::from_str(
            response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn one_call_array_preserves_response_metadata_and_returns_an_unfinished_decision() {
        let normalized = normalize(
            response(r#" [{"name":"demo.read","arguments":{"query":"Mai"}}] "#),
            &tools(),
        )
        .unwrap();
        assert_eq!(normalized["id"], "fixture-response");
        assert_eq!(normalized["model"], "fixture-xlam");
        assert_eq!(normalized["usage"]["total_tokens"], 42);
        assert_eq!(normalized["choices"][0]["finish_reason"], "stop");
        assert_eq!(decision(&normalized)["finished"], false);
        assert_eq!(decision(&normalized)["tool_calls"][0]["name"], "demo.read");
    }

    #[test]
    fn parallel_calls_preserve_nested_json_argument_values() {
        let calls = json!([
            {"name":"demo.read","arguments":{"filter":{"name":"Mai","enabled":true},"ids":[1,2]}},
            {"name":"demo.write","arguments":{"payload":{"arguments":{"name":"literal"}},"value":null}}
        ]);
        let normalized = normalize(response(&calls.to_string()), &tools()).unwrap();
        assert_eq!(decision(&normalized)["tool_calls"], calls);
        assert_eq!(decision(&normalized)["finished"], false);
    }

    #[test]
    fn malformed_unavailable_or_invalid_arrays_never_become_completion() {
        for content in [
            r#"[{"name":"demo.read","arguments":{}}"#,
            r#"[{"name":"demo.read","arguments":{}}] trailing text"#,
            r#"[{"name":"demo.read","arguments":{},"finished":true}]"#,
            r#"[{"name":"shell.run","arguments":{}}]"#,
            r#"[{"name":"demo.read","arguments":[]}]"#,
            r#"[{"name":"demo.read","arguments":null}]"#,
            r#"["demo.read"]"#,
        ] {
            assert!(normalize(response(content), &tools()).is_err(), "{content}");
        }
        let calls = vec![json!({"name":"demo.read","arguments":{}}); MAX_TOOL_CALLS + 1];
        assert!(normalize(response(&json!(calls).to_string()), &tools()).is_err());
        assert!(
            normalize(
                response(&format!("[{}", " ".repeat(MAX_CONTENT_BYTES))),
                &tools()
            )
            .is_err()
        );
    }

    #[test]
    fn empty_array_and_ordinary_completion_finish_but_truncation_does_not() {
        for content in ["[]", "Done."] {
            let normalized = normalize(response(content), &tools()).unwrap();
            assert_eq!(
                decision(&normalized),
                json!({"tool_calls":[],"finished":true})
            );
        }
        for reason in ["length", "content_filter", "error"] {
            let mut truncated = response("[]");
            truncated["choices"][0]["finish_reason"] = json!(reason);
            assert!(normalize(truncated, &tools()).is_err());
        }
        assert!(normalize(response("<tool_call>unparsed"), &tools()).is_err());
    }

    #[test]
    fn native_calls_take_precedence_and_still_enforce_the_allowed_tools() {
        let mut native = response("[malformed fallback must be ignored");
        native["choices"][0]["finish_reason"] = json!("tool_calls");
        native["choices"][0]["message"]["tool_calls"] = json!([
            {"id":"native-id","type":"function","function":{"name":"demo.read","arguments":"{\"value\":7}"}}
        ]);
        let normalized = normalize(native.clone(), &tools()).unwrap();
        assert_eq!(normalized["id"], "fixture-response");
        assert_eq!(normalized["usage"]["total_tokens"], 42);
        assert_eq!(
            decision(&normalized)["tool_calls"],
            json!([{ "name":"demo.read", "arguments":{"value":7}}])
        );
        native["choices"][0]["message"]["tool_calls"][0]["function"]["name"] = json!("shell.run");
        assert!(normalize(native, &tools()).is_err());
    }

    #[test]
    fn empty_native_array_allows_the_strict_content_fallback_and_invalid_transport_is_rejected() {
        let mut raw = response(r#"[{"name":"demo.read","arguments":{}}]"#);
        raw["choices"][0]["message"]["tool_calls"] = json!([]);
        assert_eq!(
            decision(&normalize(raw, &tools()).unwrap())["finished"],
            false
        );
        let mut malformed = response("Done.");
        malformed["choices"][0]["message"]["tool_calls"] = json!({});
        assert!(normalize(malformed, &tools()).is_err());
        assert!(normalize(json!({"choices":[]}), &tools()).is_err());
        assert!(
            normalize(
                json!({"choices":[{"finish_reason":"stop","message":null}]}),
                &tools()
            )
            .is_err()
        );
    }
}
