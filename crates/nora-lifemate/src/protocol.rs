use crate::{MAX_JSON_BYTES, MAX_TOOLS, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
pub struct InputItem {
    pub id: String,
    pub audio: String,
    pub context: Context,
    #[serde(default)]
    pub history: Vec<HistoryTurn>,
    // label and audio_text are intentionally never deserialized.
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Context {
    pub current_time: String,
    pub timezone: String,
    #[serde(default)]
    pub location: Value,
    #[serde(default)]
    pub user_profile: Value,
    pub installed_apps: BTreeMap<String, InstalledApp>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstalledApp {
    pub tools: Vec<String>,
    // In particular, an inline app state is not admitted.
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoryTurn {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub kind: String,
    pub parameters: Value,
    #[serde(default)]
    pub returns: Value,
}

#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub schema_version: String,
    pub apps: Vec<CatalogApp>,
}
#[derive(Debug, Deserialize)]
pub struct CatalogApp {
    pub id: String,
    pub tools: Vec<ToolSpec>,
}

impl InputItem {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            || self.history.len() > 64
            || self.context.installed_apps.len() > 128
            || self.context.current_time.is_empty()
            || self.context.timezone.is_empty()
        {
            return Err("invalid or oversized contest input".into());
        }
        for (app, installed) in &self.context.installed_apps {
            if installed.tools.len() > MAX_TOOLS
                || installed
                    .tools
                    .iter()
                    .any(|name| name.split_once('.').is_none_or(|(prefix, _)| prefix != app))
            {
                return Err("installed tool does not belong to its declared app".into());
            }
        }
        if self
            .history
            .iter()
            .any(|h| !matches!(h.role.as_str(), "user" | "assistant"))
        {
            return Err("unsupported history role".into());
        }
        Ok(())
    }
    pub fn allowed_tools(&self, catalog: &Catalog) -> Result<Vec<ToolSpec>> {
        let mut tools = Vec::new();
        for app in &catalog.apps {
            for tool in &app.tools {
                if tool.name == "sys.ask_missing_info"
                    || self
                        .context
                        .installed_apps
                        .get(&app.id)
                        .is_some_and(|a| a.tools.contains(&tool.name))
                {
                    tools.push(tool.clone());
                }
            }
        }
        if tools.len() > MAX_TOOLS || !tools.iter().any(|t| t.name == "sys.ask_missing_info") {
            return Err("invalid allowed tool catalog".into());
        }
        for installed in self.context.installed_apps.values() {
            if installed
                .tools
                .iter()
                .any(|name| !tools.iter().any(|t| &t.name == name))
            {
                return Err("installed tool is missing from catalog".into());
            }
        }
        Ok(tools)
    }
}

pub fn audio_path(split_directory: &Path, relative: &str) -> Result<PathBuf> {
    // This is the only dataset asset resolver. It cannot resolve environment
    // paths, absolute paths, alternate Windows streams or parent traversal.
    if relative.contains(['\\', ':']) || relative.len() > 1024 {
        return Err("invalid audio path".into());
    }
    let path = Path::new(relative);
    let parts: Vec<_> = path.components().collect();
    if parts.len() < 2
        || parts[0] != Component::Normal("audio".as_ref())
        || parts.iter().any(|p| !matches!(p, Component::Normal(_)))
        || path.extension().is_none_or(|s| s != "wav")
    {
        return Err("only relative paths under audio/ are permitted".into());
    }
    let audio_root = split_directory.join("audio").canonicalize()?;
    let full = split_directory.join(path).canonicalize()?;
    if !full.starts_with(audio_root) || !full.is_file() {
        return Err("audio asset escapes the split's audio root".into());
    }
    Ok(full)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub finished: bool,
}
impl Decision {
    pub fn parse(text: &str, tools: &[ToolSpec]) -> Result<Self> {
        if text.len() > 65_536 {
            return Err("decision exceeds output bound".into());
        }
        let decision: Self = serde_json::from_str(text)?;
        if decision.tool_calls.len() > 8
            || decision.tool_calls.iter().any(|call| {
                !call.arguments.is_object() || !tools.iter().any(|t| t.name == call.name)
            })
        {
            return Err("decision contains unavailable or invalid tool calls".into());
        }
        Ok(decision)
    }
}

pub fn decision_schema(tools: &[ToolSpec]) -> Value {
    let alternatives: Vec<_> = tools
        .iter()
        .map(|tool| {
            json!({"type":"object","properties":{
                "name":{"const":tool.name},
                "arguments":decoder_parameters(&tool.parameters)
            },"required":["name","arguments"],"additionalProperties":false})
        })
        .collect();
    json!({"type":"object","properties":{
        "tool_calls":{"type":"array","maxItems":8,"items":{"anyOf":alternatives}},
        "finished":{"type":"boolean"}
    },"required":["tool_calls","finished"],"additionalProperties":false})
}

pub fn legacy_decision_schema(tools: &[ToolSpec]) -> Value {
    json!({"type":"object","properties":{"finished":{"type":"boolean"},"tool_calls":{"type":"array","maxItems":8,"items":{
        "type":"object","properties":{"name":{"type":"string","enum":tools.iter().map(|t|&t.name).collect::<Vec<_>>()},
        "arguments":{"type":"object"}},"required":["name","arguments"],"additionalProperties":false
    }}},"required":["tool_calls","finished"],"additionalProperties":false})
}

/// Compile the catalog into decoder constraints, without changing the SDK.
/// Its x-one-of means AT LEAST one complete group, not exclusive oneOf.
fn decoder_parameters(schema: &Value) -> Value {
    let Some(original) = schema.as_object() else {
        return schema.clone();
    };
    let mut object = original.clone();
    for annotation in ["description", "x-ref", "x-match"] {
        object.remove(annotation);
    }
    if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
        for property in properties.values_mut() {
            *property = decoder_parameters(property);
        }
    }
    if let Some(items) = object.get_mut("items") {
        *items = decoder_parameters(items);
    }
    if object.get("type").and_then(Value::as_str) == Some("object") {
        object.insert("additionalProperties".into(), Value::Bool(false));
    }
    if let Some(Value::Array(groups)) = object.remove("x-one-of")
        && !groups.is_empty()
    {
        let alternatives: Vec<_> = groups
            .iter()
            .filter_map(Value::as_array)
            .map(|group| {
                let mut variant = object.clone();
                let required = variant.entry("required").or_insert_with(|| json!([]));
                if let Some(required) = required.as_array_mut() {
                    for key in group {
                        if !required.contains(key) {
                            required.push(key.clone());
                        }
                    }
                }
                constrain_required_values(&mut variant);
                Value::Object(variant)
            })
            .collect();
        return json!({"anyOf":alternatives});
    }
    constrain_required_values(&mut object);
    Value::Object(object)
}

fn constrain_required_values(schema: &mut serde_json::Map<String, Value>) {
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else {
        return;
    };
    for key in required.iter().filter_map(Value::as_str) {
        let Some(property) = properties.get_mut(key).and_then(Value::as_object_mut) else {
            continue;
        };
        // The official validator treats empty required strings/arrays as
        // missing. Optional values keep the original catalog semantics.
        let bound = match property.get("type").and_then(Value::as_str) {
            Some("string") => "minLength",
            Some("array") => "minItems",
            _ => continue,
        };
        if property.get(bound).and_then(Value::as_u64).unwrap_or(0) < 1 {
            property.insert(bound.into(), json!(1));
        }
    }
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_JSON_BYTES as u64 {
        return Err("JSON asset exceeds file bound".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_JSON_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("JSON asset exceeds bound".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contest_labels_and_inline_state_never_enter_inference() {
        let item: InputItem = serde_json::from_value(json!({"id":"sample_1","audio":"audio/a.wav",
            "audio_text":"SECRET_LABEL", "label":{"tool_calls":["SECRET_GOLD"]},
            "context":{"current_time":"2026-11-02","timezone":"Asia/Ho_Chi_Minh",
            "installed_apps":{"notes":{"tools":["notes.list"],"state":{"secret":"HIDDEN_STATE"}}}},
            "history":[{"role":"user","audio":"audio/b.wav","audio_text":"SECRET_HISTORY"}]}))
        .unwrap();
        item.validate().unwrap();
        let admitted =
            serde_json::to_string(&json!({"context":item.context,"history":item.history})).unwrap();
        assert!(!admitted.contains("SECRET"));
        assert!(!admitted.contains("HIDDEN_STATE"));
    }
    #[test]
    fn audio_paths_cannot_reach_environment_or_parent_or_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("audio")).unwrap();
        std::fs::write(dir.path().join("audio/a.wav"), b"fixture").unwrap();
        assert!(audio_path(dir.path(), "audio/a.wav").is_ok());
        for name in [
            "train_environment/x/state.json",
            "../audio/a.wav",
            "audio/../a.wav",
            "/audio/a.wav",
            "audio/a.wav:stream",
            "audio\\a.wav",
        ] {
            assert!(audio_path(dir.path(), name).is_err(), "{name}");
        }
    }
    #[test]
    fn structured_decisions_reject_unavailable_or_non_object_arguments() {
        let tools = vec![ToolSpec {
            name: "notes.list".into(),
            description: String::new(),
            kind: "read".into(),
            parameters: json!({}),
            returns: Value::Null,
        }];
        assert!(Decision::parse(r#"{"tool_calls":[]}"#, &tools).is_ok());
        assert!(
            Decision::parse(
                r#"{"tool_calls":[{"name":"shell.exec","arguments":{}}]}"#,
                &tools
            )
            .is_err()
        );
        assert!(
            Decision::parse(
                r#"{"tool_calls":[{"name":"notes.list","arguments":"{}"}]}"#,
                &tools
            )
            .is_err()
        );
        assert!(Decision::parse(r#"{"tool_calls":[],"state_hash":"forged"}"#, &tools).is_err());
    }

    #[test]
    fn decoder_pairs_each_tool_with_its_own_arguments_and_keeps_catalog_unchanged() {
        let parameters = json!({"type":"object","properties":{
            "content":{"type":"string"},
            "count":{"type":"integer","minimum":1}
        },"required":["content"]});
        let tool = ToolSpec {
            name: "toy.send".into(),
            description: String::new(),
            kind: "write".into(),
            parameters: parameters.clone(),
            returns: Value::Null,
        };
        let schema = decision_schema(std::slice::from_ref(&tool));
        let variant = &schema["properties"]["tool_calls"]["items"]["anyOf"][0];
        assert_eq!(variant["properties"]["name"]["const"], "toy.send");
        let args = &variant["properties"]["arguments"];
        assert_eq!(args["properties"]["content"]["minLength"], 1);
        assert_eq!(args["properties"]["count"]["minimum"], 1);
        assert_eq!(args["additionalProperties"], false);
        assert_eq!(tool.parameters, parameters);
    }

    #[test]
    fn catalog_alternative_groups_allow_either_or_both_and_nonempty_required_values() {
        let schema = decoder_parameters(&json!({"type":"object","properties":{
            "contact_id":{"type":"string"},"phone":{"type":"string"},
            "content":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}}
        },"required":["content","tags"],"x-one-of":[["contact_id"],["phone"]]}));
        let alternatives = schema["anyOf"].as_array().unwrap();
        assert_eq!(alternatives.len(), 2);
        for (index, selected) in ["contact_id", "phone"].iter().enumerate() {
            let variant = &alternatives[index];
            assert_eq!(variant["required"], json!(["content", "tags", selected]));
            assert_eq!(variant["properties"][selected]["minLength"], 1);
            assert_eq!(variant["properties"]["tags"]["minItems"], 1);
            assert!(variant["properties"].get("contact_id").is_some());
            assert!(variant["properties"].get("phone").is_some());
        }
        assert!(schema.get("oneOf").is_none());
        assert!(schema.get("properties").is_none());
    }
}
