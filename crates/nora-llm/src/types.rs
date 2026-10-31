use std::{
    collections::BTreeSet,
    fmt,
    io::{self, Write},
};

use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    ApiError, MAX_INPUT_ITEMS, MAX_OUTPUT_ITEMS, MAX_OUTPUT_TEXT_BYTES, MAX_OUTPUT_TOKENS,
    MAX_REQUEST_BYTES, MAX_TOOL_ARGUMENT_BYTES, MAX_TOOL_CALLS, MAX_TOOLS, Result,
    config::ApiConfig,
};

/// Caller-owned input history. Do not serialize/log it as application telemetry.
/// Opaque reasoning/output items may be appended for a tool continuation, but
/// must never be presented as user-facing progress or treated as authority.
pub struct TurnRequest {
    pub instructions: String,
    pub input: Vec<Value>,
    pub tools: Vec<Value>,
    pub max_output_tokens: Option<u32>,
}

impl fmt::Debug for TurnRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnRequest")
            .field("instruction_bytes", &self.instructions.len())
            .field("input_items", &self.input.len())
            .field("tools", &self.tools.len())
            .field("max_output_tokens", &self.max_output_tokens)
            .finish()
    }
}

impl TurnRequest {
    pub fn user(instructions: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            instructions: instructions.into(),
            input: vec![json!({"role":"user","content":text.into()})],
            tools: Vec::new(),
            max_output_tokens: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Completed,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    MaxOutputTokens,
    ContentFilter,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    /// Includes reasoning tokens. Do not add reasoning_tokens a second time.
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub reasoning_tokens: Option<u64>,
}

#[derive(Clone, PartialEq)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

impl fmt::Debug for ToolCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolCall")
            .field("call_id", &self.call_id)
            .field("name", &self.name)
            .field("arguments", &"[NOT LOGGED]")
            .finish()
    }
}

pub struct TurnResponse {
    pub id: String,
    pub text: String,
    /// Always empty when status is Incomplete. The caller must also check status
    /// before showing text as a completed answer or dispatching any action.
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
    /// Bounded provider output for continuation/citation handling. Contains
    /// opaque reasoning state and potentially private content. Never log this.
    pub output_items: Vec<Value>,
    pub status: ResponseStatus,
    pub incomplete_reason: Option<IncompleteReason>,
    pub refused: bool,
}

impl fmt::Debug for TurnResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnResponse")
            .field("id", &self.id)
            .field("text_bytes", &self.text.len())
            .field("tool_calls", &self.tool_calls.len())
            .field("usage", &self.usage)
            .field("output_items", &self.output_items.len())
            .field("status", &self.status)
            .field("incomplete_reason", &self.incomplete_reason)
            .field("refused", &self.refused)
            .finish()
    }
}

pub(crate) struct PreparedRequest {
    pub body: Vec<u8>,
    pub functions: BTreeSet<String>,
    pub web_search: bool,
}

pub(crate) fn prepare(config: &ApiConfig, request: &TurnRequest) -> Result<PreparedRequest> {
    if request.instructions.len() > 32_768
        || request.instructions.trim().is_empty()
        || request.input.is_empty()
        || request.input.len() > MAX_INPUT_ITEMS
        || request.tools.len() > MAX_TOOLS
    {
        return Err(ApiError::Limit("instructions, input items or tools"));
    }
    let output_limit = request
        .max_output_tokens
        .unwrap_or(config.max_output_tokens);
    if !(64..=MAX_OUTPUT_TOKENS).contains(&output_limit)
        || output_limit > config.max_output_tokens
        || u64::from(output_limit) > config.goal_limits.max_output_tokens
    {
        return Err(ApiError::Limit("per-turn output tokens"));
    }
    let mut functions = BTreeSet::new();
    let mut web_search = false;
    for tool in &request.tools {
        let object = tool
            .as_object()
            .ok_or(ApiError::Protocol("tool must be an object"))?;
        match object.get("type").and_then(Value::as_str) {
            Some("function") => {
                let name = field_string(tool, "name", 64)?;
                if !valid_function_name(name) || !functions.insert(name.to_owned()) {
                    return Err(ApiError::Protocol("invalid or duplicate function name"));
                }
                if object.get("strict") != Some(&Value::Bool(true)) {
                    return Err(ApiError::Protocol("function tools must use strict schemas"));
                }
                field_string(tool, "description", 8192)?;
                let schema = object
                    .get("parameters")
                    .ok_or(ApiError::Protocol("missing function schema"))?;
                if schema.get("type").and_then(Value::as_str) != Some("object")
                    || schema.get("additionalProperties") != Some(&Value::Bool(false))
                    || !schema.get("properties").is_some_and(Value::is_object)
                    || !schema.get("required").is_some_and(Value::is_array)
                {
                    return Err(ApiError::Protocol(
                        "function parameters must be a closed object schema",
                    ));
                }
            }
            Some("web_search") if !web_search => web_search = true,
            _ => return Err(ApiError::Protocol("unsupported or duplicate built-in tool")),
        }
    }
    for input in &request.input {
        let object = input
            .as_object()
            .ok_or(ApiError::Protocol("input item must be an object"))?;
        let kind = object.get("type").and_then(Value::as_str);
        if !matches!(
            kind,
            None | Some(
                "message"
                    | "function_call"
                    | "function_call_output"
                    | "reasoning"
                    | "web_search_call"
            )
        ) {
            return Err(ApiError::Protocol("unsupported input item type"));
        }
        if kind.is_none() && !object.contains_key("role") {
            return Err(ApiError::Protocol("input message requires a role"));
        }
    }
    bound_json_tree(&request.input)?;
    bound_json_tree(&request.tools)?;
    let mut include = vec!["reasoning.encrypted_content"];
    if web_search {
        include.push("web_search_call.action.sources");
    }
    // Borrow caller data: a bounded writer must reject excessive bytes before
    // a JSON macro or generic to_value can clone an oversized input tree.
    #[derive(Serialize)]
    struct Reasoning<'a> {
        effort: &'a str,
    }
    #[derive(Serialize)]
    struct Body<'a> {
        model: &'a str,
        reasoning: Reasoning<'a>,
        instructions: &'a str,
        input: &'a [Value],
        tools: &'a [Value],
        include: Vec<&'static str>,
        max_output_tokens: u32,
        parallel_tool_calls: bool,
        store: bool,
        stream: bool,
        truncation: &'static str,
    }
    let body = Body {
        model: &config.model,
        reasoning: Reasoning {
            effort: &config.reasoning_effort,
        },
        instructions: &request.instructions,
        input: &request.input,
        tools: &request.tools,
        include,
        max_output_tokens: output_limit,
        parallel_tool_calls: false,
        store: false,
        stream: false,
        truncation: "disabled",
    };
    let mut writer = BoundedWriter::new(MAX_REQUEST_BYTES);
    serde_json::to_writer(&mut writer, &body)
        .map_err(|_| ApiError::Limit("encoded request bytes"))?;
    Ok(PreparedRequest {
        body: writer.bytes,
        functions,
        web_search,
    })
}

pub(crate) fn parse_response(
    bytes: &[u8],
    model: &str,
    prepared: &PreparedRequest,
) -> Result<TurnResponse> {
    let mut raw: Value =
        serde_json::from_slice(bytes).map_err(|_| ApiError::Protocol("invalid response JSON"))?;
    let object = raw
        .as_object_mut()
        .ok_or(ApiError::Protocol("response is not an object"))?;
    let id = field_string_map(object, "id", 256)?.to_owned();
    if !valid_id(&id) {
        return Err(ApiError::Protocol("invalid response identifier"));
    }
    let actual_model = field_string_map(object, "model", 256)?;
    if actual_model != model
        && !actual_model
            .strip_prefix(model)
            .is_some_and(|s| s.starts_with('-'))
    {
        return Err(ApiError::Protocol("provider returned a different model"));
    }
    if object.get("error").is_some_and(|v| !v.is_null()) {
        return Err(ApiError::Protocol("provider reported a response error"));
    }
    let status = match object.get("status").and_then(Value::as_str) {
        Some("completed") => ResponseStatus::Completed,
        Some("incomplete") => ResponseStatus::Incomplete,
        _ => return Err(ApiError::Protocol("unexpected provider completion status")),
    };
    let incomplete_reason = if status == ResponseStatus::Incomplete {
        Some(
            match object
                .get("incomplete_details")
                .and_then(|v| v.get("reason"))
                .and_then(Value::as_str)
            {
                Some("max_output_tokens") => IncompleteReason::MaxOutputTokens,
                Some("content_filter") => IncompleteReason::ContentFilter,
                _ => IncompleteReason::Other,
            },
        )
    } else {
        None
    };
    let usage = parse_usage(
        object
            .get("usage")
            .ok_or(ApiError::Protocol("missing usage accounting"))?,
    )?;
    let items = object
        .remove("output")
        .and_then(|v| {
            if let Value::Array(a) = v {
                Some(a)
            } else {
                None
            }
        })
        .ok_or(ApiError::Protocol("missing output items"))?;
    if items.len() > MAX_OUTPUT_ITEMS {
        return Err(ApiError::Limit("output items"));
    }
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut call_ids = BTreeSet::new();
    let mut refused = false;
    for item in &items {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                if item.get("role").and_then(Value::as_str) != Some("assistant") {
                    return Err(ApiError::Protocol("unexpected output message role"));
                }
                let parts = item
                    .get("content")
                    .and_then(Value::as_array)
                    .ok_or(ApiError::Protocol("missing message content"))?;
                if parts.len() > 64 {
                    return Err(ApiError::Limit("message parts"));
                }
                for part in parts {
                    let next = match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => field_string(part, "text", MAX_OUTPUT_TEXT_BYTES)?,
                        Some("refusal") => {
                            refused = true;
                            field_string(part, "refusal", MAX_OUTPUT_TEXT_BYTES)?
                        }
                        _ => return Err(ApiError::Protocol("unsupported output message content")),
                    };
                    let extra = usize::from(!text.is_empty());
                    if text
                        .len()
                        .checked_add(next.len())
                        .and_then(|n| n.checked_add(extra))
                        .is_none_or(|n| n > MAX_OUTPUT_TEXT_BYTES)
                    {
                        return Err(ApiError::Limit("output text bytes"));
                    }
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(next);
                }
            }
            Some("function_call") => {
                // Incomplete/truncated arguments are not parsed or dispatched.
                if status == ResponseStatus::Incomplete {
                    continue;
                }
                if item
                    .get("status")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s != "completed")
                {
                    return Err(ApiError::Protocol("function call is not complete"));
                }
                let call_id = field_string(item, "call_id", 256)?;
                let name = field_string(item, "name", 64)?;
                if !valid_id(call_id)
                    || !call_ids.insert(call_id.to_owned())
                    || !prepared.functions.contains(name)
                {
                    return Err(ApiError::Protocol(
                        "unknown function or duplicate/invalid call identifier",
                    ));
                }
                let encoded = field_string(item, "arguments", MAX_TOOL_ARGUMENT_BYTES)?;
                let arguments: Value = serde_json::from_str(encoded)
                    .map_err(|_| ApiError::Protocol("invalid function arguments JSON"))?;
                if !arguments.is_object() {
                    return Err(ApiError::Protocol("function arguments must be an object"));
                }
                if tool_calls.len() >= MAX_TOOL_CALLS {
                    return Err(ApiError::Limit("function call count"));
                }
                tool_calls.push(ToolCall {
                    call_id: call_id.to_owned(),
                    name: name.to_owned(),
                    arguments,
                });
            }
            Some("reasoning") => {}
            Some("web_search_call") if prepared.web_search => {}
            _ => {
                return Err(ApiError::Protocol(
                    "provider returned an unrequested output type",
                ));
            }
        }
    }
    if refused && !tool_calls.is_empty() {
        return Err(ApiError::Protocol("refusal cannot dispatch function calls"));
    }
    if status == ResponseStatus::Completed && tool_calls.is_empty() && text.trim().is_empty() {
        return Err(ApiError::Protocol(
            "completed response has no answer or function call",
        ));
    }
    Ok(TurnResponse {
        id,
        text,
        tool_calls,
        usage,
        output_items: items,
        status,
        incomplete_reason,
        refused,
    })
}

fn parse_usage(raw: &Value) -> Result<Usage> {
    let get = |key: &str| {
        raw.get(key)
            .and_then(Value::as_u64)
            .ok_or(ApiError::Protocol("invalid token usage"))
    };
    let input_tokens = get("input_tokens")?;
    let output_tokens = get("output_tokens")?;
    let total_tokens = get("total_tokens")?;
    let reasoning_tokens = raw
        .get("output_tokens_details")
        .and_then(|v| v.get("reasoning_tokens"))
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_u64()
                .ok_or(ApiError::Protocol("invalid reasoning usage"))
        })
        .transpose()?;
    if input_tokens.checked_add(output_tokens) != Some(total_tokens)
        || total_tokens > 4_000_000
        || reasoning_tokens.is_some_and(|n| n > output_tokens)
    {
        return Err(ApiError::Protocol("inconsistent token accounting"));
    }
    Ok(Usage {
        input_tokens,
        output_tokens,
        total_tokens,
        reasoning_tokens,
    })
}

fn field_string<'a>(value: &'a Value, field: &str, max: usize) -> Result<&'a str> {
    let text = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(ApiError::Protocol(
            "required response/tool field is not text",
        ))?;
    if text.len() > max {
        return Err(ApiError::Limit("response/tool text field"));
    }
    Ok(text)
}

fn field_string_map<'a>(
    map: &'a serde_json::Map<String, Value>,
    field: &str,
    max: usize,
) -> Result<&'a str> {
    let text = map
        .get(field)
        .and_then(Value::as_str)
        .ok_or(ApiError::Protocol("required response field is not text"))?;
    if text.is_empty() || text.len() > max {
        return Err(ApiError::Limit("response identifier/model"));
    }
    Ok(text)
}

fn valid_function_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

fn valid_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
}

fn bound_json_tree(values: &[Value]) -> Result<()> {
    let mut pending: Vec<(&Value, usize)> = values.iter().map(|v| (v, 0)).collect();
    let mut nodes = 0usize;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if nodes > 65_536 || depth > 64 {
            return Err(ApiError::Limit("JSON node/depth budget"));
        }
        match value {
            Value::Array(a) => {
                if a.len() > 4096 || pending.len() + a.len() > 65_536 {
                    return Err(ApiError::Limit("JSON array budget"));
                }
                pending.extend(a.iter().map(|v| (v, depth + 1)));
            }
            Value::Object(o) => {
                if o.len() > 1024 || pending.len() + o.len() > 65_536 {
                    return Err(ApiError::Limit("JSON object budget"));
                }
                pending.extend(o.values().map(|v| (v, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max: usize,
}

impl BoundedWriter {
    fn new(max: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(max.min(4096)),
            max,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(input.len())
            .is_none_or(|n| n > self.max)
        {
            return Err(io::Error::other("bounded JSON output exceeded"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ApiConfig {
        ApiConfig::parse(
            b"NORA_LLM_API_KEY=synthetic-key\nNORA_LLM_MODEL=gpt-6-luna",
            false,
        )
        .unwrap()
    }

    pub(super) fn function() -> Value {
        json!({"type":"function","name":"read_file","description":"Read an approved path","strict":true,"parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}})
    }

    fn fixture(output: Vec<Value>) -> Value {
        json!({"id":"resp_synthetic","model":"gpt-6-luna","status":"completed","usage":{"input_tokens":10,"output_tokens":8,"total_tokens":18,"output_tokens_details":{"reasoning_tokens":3}},"output":output})
    }

    fn answer(text: &str) -> Value {
        json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":text,"annotations":[]}]})
    }

    #[test]
    fn exact_model_reasoning_nonstorage_and_explicit_search_includes() {
        let config = config();
        let mut request = TurnRequest::user("Answer briefly in Vietnamese.", "Xin chào");
        request.tools = vec![
            function(),
            json!({"type":"web_search","search_context_size":"low"}),
        ];
        let prepared = prepare(&config, &request).unwrap();
        let body: Value = serde_json::from_slice(&prepared.body).unwrap();
        assert_eq!(body["model"], "gpt-6-luna");
        assert_eq!(body["reasoning"]["effort"], "xhigh");
        assert_eq!(body["store"], false);
        assert_eq!(body["truncation"], "disabled");
        assert_eq!(body["parallel_tool_calls"], false);
        assert!(
            body["include"]
                .as_array()
                .unwrap()
                .contains(&json!("web_search_call.action.sources"))
        );
        assert!(!format!("{request:?}").contains("Xin chào"));
    }

    #[test]
    fn output_text_tool_and_opaque_reasoning_have_separate_surfaces() {
        let config = config();
        let mut request = TurnRequest::user("Use approved tools.", "Read my note");
        request.tools.push(function());
        let prepared = prepare(&config, &request).unwrap();
        let raw = fixture(vec![
            json!({"type":"reasoning","id":"rs_1","encrypted_content":"opaque-synthetic","summary":[]}),
            json!({"type":"function_call","status":"completed","call_id":"call_1","name":"read_file","arguments":"{\"path\":\"private.md\"}"}),
        ]);
        let response =
            parse_response(&serde_json::to_vec(&raw).unwrap(), "gpt-6-luna", &prepared).unwrap();
        assert_eq!(response.status, ResponseStatus::Completed);
        assert_eq!(response.tool_calls[0].arguments["path"], "private.md");
        assert_eq!(response.output_items.len(), 2);
        assert!(response.text.is_empty());
        assert!(!format!("{response:?}").contains("opaque-synthetic"));
        assert!(!format!("{:?}", response.tool_calls[0]).contains("private.md"));
        assert_eq!(response.usage.reasoning_tokens, Some(3));
    }

    #[test]
    fn incomplete_calls_never_dispatch_and_unknown_tools_or_model_fail_closed() {
        let config = config();
        let mut request = TurnRequest::user("Use approved tools.", "Read my note");
        request.tools.push(function());
        let prepared = prepare(&config, &request).unwrap();
        let mut raw = fixture(vec![
            json!({"type":"function_call","call_id":"call_1","name":"read_file","arguments":"{\"path\":"}),
        ]);
        raw["status"] = json!("incomplete");
        raw["incomplete_details"] = json!({"reason":"max_output_tokens"});
        let response =
            parse_response(&serde_json::to_vec(&raw).unwrap(), "gpt-6-luna", &prepared).unwrap();
        assert!(response.tool_calls.is_empty());
        assert_eq!(
            response.incomplete_reason,
            Some(IncompleteReason::MaxOutputTokens)
        );
        raw = fixture(vec![
            json!({"type":"function_call","call_id":"call_1","name":"unapproved","arguments":"{}"}),
        ]);
        assert!(
            parse_response(&serde_json::to_vec(&raw).unwrap(), "gpt-6-luna", &prepared).is_err()
        );
        raw = fixture(vec![answer("Nora")]);
        raw["model"] = json!("gpt-6-astra");
        assert!(
            parse_response(&serde_json::to_vec(&raw).unwrap(), "gpt-6-luna", &prepared).is_err()
        );
    }

    #[test]
    fn malformed_arguments_duplicates_and_inconsistent_usage_are_rejected() {
        let config = config();
        let mut request = TurnRequest::user("Use approved tools.", "Read my note");
        request.tools.push(function());
        let prepared = prepare(&config, &request).unwrap();
        for arguments in ["not-json", "[]", "null"] {
            let raw = fixture(vec![
                json!({"type":"function_call","call_id":"call_1","name":"read_file","arguments":arguments}),
            ]);
            assert!(
                parse_response(&serde_json::to_vec(&raw).unwrap(), "gpt-6-luna", &prepared)
                    .is_err()
            );
        }
        let call =
            json!({"type":"function_call","call_id":"same","name":"read_file","arguments":"{}"});
        let raw = fixture(vec![call.clone(), call]);
        assert!(
            parse_response(&serde_json::to_vec(&raw).unwrap(), "gpt-6-luna", &prepared).is_err()
        );
        let mut raw = fixture(vec![answer("Nora")]);
        raw["usage"]["total_tokens"] = json!(99);
        assert!(
            parse_response(&serde_json::to_vec(&raw).unwrap(), "gpt-6-luna", &prepared).is_err()
        );
    }

    #[test]
    fn oversized_inputs_and_unsafe_schema_are_rejected_before_network() {
        let config = config();
        let mut request = TurnRequest::user("Answer briefly.", "x".repeat(MAX_REQUEST_BYTES));
        assert_eq!(
            prepare(&config, &request).err(),
            Some(ApiError::Limit("encoded request bytes"))
        );
        request = TurnRequest::user("Answer briefly.", "hello");
        request.max_output_tokens = Some(8192);
        assert!(prepare(&config, &request).is_err());
        request.max_output_tokens = None;
        let mut tool = function();
        tool["strict"] = json!(false);
        request.tools.push(tool);
        assert!(prepare(&config, &request).is_err());
    }
}
