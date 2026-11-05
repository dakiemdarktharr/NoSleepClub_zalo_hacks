//! Experimental, catalog-only function name aliases. All SDK authority and
//! exact argument schemas remain attached to the original function names.
use crate::{
    Result,
    protocol::{Catalog, HistoryTurn, ToolCall, ToolSpec},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const ASK: &str = "sys.ask_missing_info";
const VERBS: &[&str] = &[
    "list", "create", "get", "send", "call", "search", "start", "stop", "read", "update", "delete",
    "cancel", "complete", "add",
];

#[derive(Debug, Clone)]
pub struct NameMap {
    forward: BTreeMap<String, String>,
    reverse: BTreeMap<String, String>,
}

fn identifier(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes.next().is_some_and(|c| c.is_ascii_lowercase())
        && bytes.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}
fn parts(name: &str) -> Result<(&str, &str)> {
    let (app, method) = name.split_once('.').ok_or("invalid catalog tool name")?;
    if !identifier(app) || !identifier(method) {
        return Err("invalid catalog tool name".into());
    }
    Ok((app, method))
}
fn preferred(name: &str) -> Result<String> {
    let (app, method) = parts(name)?;
    if name == ASK || !VERBS.contains(&method) {
        return Ok(name.to_owned());
    }
    // ASCII-only as validated above: slicing by one byte is safe.
    let noun = if app.len() > 4 && app.ends_with('s') && !app.ends_with("ss") {
        &app[..app.len() - 1]
    } else {
        app
    };
    Ok(format!("{app}.{method}_{noun}"))
}

impl NameMap {
    pub fn from_catalog(catalog: &Catalog) -> Result<Self> {
        for app in &catalog.apps {
            for tool in &app.tools {
                if parts(&tool.name)?.0 != app.id {
                    return Err("tool is outside catalog app namespace".into());
                }
            }
        }
        let names: Vec<_> = catalog
            .apps
            .iter()
            .flat_map(|a| a.tools.iter().map(|t| t.name.clone()))
            .collect();
        let mut preferences = BTreeMap::new();
        for name in &names {
            preferences.insert(name.clone(), preferred(name)?);
        }
        Self::from_preferences(&names, &preferences)
    }
    fn from_preferences(names: &[String], preferences: &BTreeMap<String, String>) -> Result<Self> {
        let all: BTreeSet<_> = names.iter().cloned().collect();
        if all.len() != names.len() || !all.contains(ASK) {
            return Err("invalid alias catalog".into());
        }
        let mut forward = BTreeMap::new();
        for name in &all {
            let (app, _) = parts(name)?;
            let requested = preferences.get(name).unwrap_or(name);
            let safe = parts(requested).is_ok_and(|(candidate_app, _)| candidate_app == app)
                && (!all.contains(requested) || requested == name)
                && (name != ASK || requested == name);
            forward.insert(
                name.clone(),
                if safe {
                    requested.clone()
                } else {
                    name.clone()
                },
            );
        }
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, alias) in &forward {
            groups.entry(alias.clone()).or_default().push(name.clone());
        }
        for group in groups.values().filter(|g| g.len() > 1) {
            for name in group {
                forward.insert(name.clone(), name.clone());
            }
        }
        let reverse: BTreeMap<_, _> = forward
            .iter()
            .map(|(a, b)| (b.clone(), a.clone()))
            .collect();
        if reverse.len() != forward.len() || forward.get(ASK).map(String::as_str) != Some(ASK) {
            return Err("invalid alias bijection".into());
        }
        Ok(Self { forward, reverse })
    }
    pub fn to_model(&self, name: &str) -> Result<&str> {
        self.forward
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| "unknown official tool".into())
    }
    fn to_sdk(&self, name: &str, allowed: &[ToolSpec]) -> Result<&str> {
        let original = self.reverse.get(name).ok_or("unknown model tool alias")?;
        if !allowed.iter().any(|t| &t.name == original) {
            return Err("model tool is not allowed".into());
        }
        Ok(original)
    }
    pub fn model_tools(&self, tools: &[ToolSpec]) -> Result<Vec<ToolSpec>> {
        tools
            .iter()
            .map(|tool| {
                let mut t = tool.clone();
                t.name = self.to_model(&t.name)?.to_owned();
                Ok(t)
            })
            .collect()
    }
    /// Convert model proposal to official SDK name before all security checks.
    pub fn sdk_call(&self, call: &ToolCall, allowed: &[ToolSpec]) -> Result<ToolCall> {
        let mut out = call.clone();
        out.name = self.to_sdk(&call.name, allowed)?.to_owned();
        if out.name == ASK
            && let Some(target) = out.arguments.get_mut("tool_name")
        {
            let alias = target.as_str().ok_or("invalid ask target")?;
            *target = Value::String(self.to_sdk(alias, allowed)?.to_owned());
        }
        Ok(out)
    }
    /// Convert an actual official call for display; arguments are opaque other
    /// than sys.ask's typed tool_name field. Does not authorize any SDK call.
    pub fn model_call(&self, call: &ToolCall) -> Result<ToolCall> {
        let mut out = call.clone();
        out.name = self.to_model(&call.name)?.to_owned();
        if out.name == ASK
            && let Some(target) = out.arguments.get_mut("tool_name")
        {
            let original = target.as_str().ok_or("invalid ask target")?;
            *target = Value::String(self.to_model(original)?.to_owned());
        }
        Ok(out)
    }
    /// Historic calls are untrusted data: rewrite known structured names
    /// only; preserve unknown and malformed entries as opaque history rather
    /// than granting them permissions or failing a whole sample.
    pub fn model_history(&self, turns: &mut [HistoryTurn]) {
        for turn in turns {
            if turn.role != "assistant" {
                continue;
            }
            let Some(calls) = turn.tool_calls.as_mut().and_then(Value::as_array_mut) else {
                continue;
            };
            for value in calls {
                let Some(object) = value.as_object_mut() else {
                    continue;
                };
                let Some(name) = object.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let Some(alias) = self.forward.get(name) else {
                    continue;
                };
                let is_ask = name == ASK;
                object.insert("name".into(), Value::String(alias.clone()));
                if is_ask
                    && let Some(target) = object
                        .get_mut("arguments")
                        .and_then(|a| a.get_mut("tool_name"))
                    && let Some(alias) = target.as_str().and_then(|v| self.forward.get(v))
                {
                    *target = Value::String(alias.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn spec(n: &str) -> ToolSpec {
        ToolSpec {
            name: n.into(),
            kind: "write".into(),
            description: "example".into(),
            parameters: json!({"type":"object","properties":{"id":{"type":"string","x-ref":"other.ids"}},"x-one-of":[["id"]]}),
            returns: json!({"raw":true}),
        }
    }
    fn fake(names: &[&str]) -> NameMap {
        let names: Vec<_> = names.iter().map(|x| x.to_string()).collect();
        let preferences = names
            .iter()
            .map(|n| (n.clone(), preferred(n).unwrap()))
            .collect();
        NameMap::from_preferences(&names, &preferences).unwrap()
    }
    #[test]
    fn fixed_rule_and_unknowns_are_deterministic() {
        let m = fake(&["contacts.search", "sms.send", ASK]);
        assert_eq!(
            m.to_model("contacts.search").unwrap(),
            "contacts.search_contact"
        );
        assert_eq!(m.to_model("sms.send").unwrap(), "sms.send_sms");
        assert!(m.to_model("other.hidden").is_err());
        assert_eq!(m.to_model(ASK).unwrap(), ASK);
    }
    #[test]
    fn collisions_and_new_originals_fall_back_to_identity() {
        let n = vec![
            "x.a".to_string(),
            "x.b".to_string(),
            "x.new".to_string(),
            ASK.to_string(),
        ];
        let mut p = BTreeMap::new();
        p.insert("x.a".into(), "x.new".into());
        p.insert("x.b".into(), "x.new".into());
        let m = NameMap::from_preferences(&n, &p).unwrap();
        for name in &n {
            assert_eq!(m.to_model(name).unwrap(), name);
        }
        p.insert("x.a".into(), "x.shared".into());
        p.insert("x.b".into(), "x.shared".into());
        let m = NameMap::from_preferences(&n, &p).unwrap();
        for name in &n {
            assert_eq!(m.to_model(name).unwrap(), name);
        }
    }
    #[test]
    fn schemas_and_args_survive_bidirectional_mapping() {
        let m = fake(&["contacts.search", ASK]);
        let original = spec("contacts.search");
        let mapped = m.model_tools(std::slice::from_ref(&original)).unwrap();
        assert_eq!(original.parameters, mapped[0].parameters);
        assert_eq!(original.returns, mapped[0].returns);
        let sdk = ToolCall {
            name: "contacts.search".into(),
            arguments: json!({"id":"observed_id","extra":{"name":"contacts.search"}}),
        };
        assert_eq!(
            m.sdk_call(
                &m.model_call(&sdk).unwrap(),
                std::slice::from_ref(&original)
            )
            .unwrap(),
            sdk
        );
    }
    #[test]
    fn clarification_target_maps_but_never_expands_authority() {
        let m = fake(&["contacts.search", ASK]);
        let tools = vec![spec("contacts.search"), spec(ASK)];
        let call = ToolCall {
            name: ASK.into(),
            arguments: json!({"tool_name":"contacts.search","params":["id"]}),
        };
        let alias = m.model_call(&call).unwrap();
        assert_eq!(alias.arguments["tool_name"], "contacts.search_contact");
        assert_eq!(m.sdk_call(&alias, &tools).unwrap(), call);
        assert!(m.sdk_call(&alias, &[spec(ASK)]).is_err());
        assert!(
            m.sdk_call(
                &ToolCall {
                    name: "contacts.search_contact".into(),
                    arguments: json!({})
                },
                &[spec(ASK)]
            )
            .is_err()
        );
    }
    #[test]
    fn history_rewrites_only_known_structured_calls_and_leaves_results_opaque() {
        let m = fake(&["contacts.search", ASK]);
        let raw = json!({"role":"assistant","text":"contacts.search","tool_calls":[
            {"name":"contacts.search","arguments":{"nested":{"name":"contacts.search"}}},
            {"name":ASK,"arguments":{"tool_name":"contacts.search","params":["id"]}},
            {"name":"outside.unknown","arguments":{}}
        ],"tool_results":[{"name":"contacts.search","tool_name":"contacts.search","id":"id_spoof"}]});
        let mut turns: Vec<HistoryTurn> = vec![serde_json::from_value(raw.clone()).unwrap()];
        m.model_history(&mut turns);
        let out = serde_json::to_value(&turns[0]).unwrap();
        assert_eq!(out["tool_calls"][0]["name"], "contacts.search_contact");
        assert_eq!(
            out["tool_calls"][0]["arguments"]["nested"]["name"],
            "contacts.search"
        );
        assert_eq!(
            out["tool_calls"][1]["arguments"]["tool_name"],
            "contacts.search_contact"
        );
        assert_eq!(out["tool_calls"][1]["arguments"]["params"], json!(["id"]));
        assert_eq!(out["tool_calls"][2]["name"], "outside.unknown");
        assert_eq!(out["tool_results"], raw["tool_results"]);
        assert_eq!(out["text"], raw["text"]);
    }
}
