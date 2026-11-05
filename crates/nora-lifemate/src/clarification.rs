//! Opt-in catalog-bound clarification. Field membership is structural evidence,
//! not proof that information is missing or that asking matches user intent.
use crate::protocol::{ToolCall, ToolSpec};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const ASK: &str = "sys.ask_missing_info";
const ENUM_BYTE_BUDGET: usize = 256 * 1024;

// llama.cpp does not implement uniqueItems. Enumerate every nonempty ordered
// subset for small schemas instead of relying on a silently ignored keyword.
// Keep every order (the SDK receipt/metric may distinguish them), optional
// fields, and larger schemas. The latter retain an independent runtime guard.
fn params_schema(names: &[&str], budget: &mut usize) -> Value {
    fn append<'a>(
        names: &[&'a str],
        prefix: &mut Vec<&'a str>,
        used: &mut [bool],
        variants: &mut Vec<Value>,
        bytes: &mut usize,
        limit: usize,
    ) -> bool {
        for index in 0..names.len() {
            if used[index] {
                continue;
            }
            used[index] = true;
            prefix.push(names[index]);
            let value = json!(prefix);
            *bytes += value.to_string().len() + 1;
            if *bytes > limit {
                return false;
            }
            variants.push(value);
            if !append(names, prefix, used, variants, bytes, limit) {
                return false;
            }
            prefix.pop();
            used[index] = false;
        }
        true
    }
    if !names.is_empty() && names.len() <= 6 && names.iter().all(|name| name.len() <= 64) {
        let mut variants = Vec::new();
        let mut bytes = 0;
        if append(
            names,
            &mut Vec::new(),
            &mut vec![false; names.len()],
            &mut variants,
            &mut bytes,
            *budget,
        ) {
            *budget -= bytes;
            return json!({"type":"array","enum":variants});
        }
    }
    // Avoid factorial/large-string expansion on a new or adversarial catalog.
    // This schema keeps all valid choices; rejection() enforces uniqueness.
    json!({"type":"array","minItems":1,"maxItems":names.len(),"uniqueItems":true,
        "items":{"type":"string","enum":names}})
}

/// Restrict only the decoder's clarification alternative. SDK schemas and
/// permissions stay untouched. Every supplied (including optional) field is
/// retained: requiredness alone cannot establish what the user needs to supply.
pub fn decision_schema(tools: &[ToolSpec]) -> Value {
    let mut schema = crate::protocol::decision_schema(tools);
    let mut enum_budget = ENUM_BYTE_BUDGET;
    let targets: Vec<_> = tools
        .iter()
        .filter(|t| t.name != ASK)
        .filter_map(|tool| {
            let properties = tool.parameters.get("properties")?.as_object()?;
            if properties.is_empty() {
                return None;
            }
            let names: Vec<_> = properties.keys().map(String::as_str).collect();
            let params = params_schema(&names, &mut enum_budget);
            Some(json!({"type":"object","properties":{
            "tool_name":{"const":tool.name},
            "params":params
        },"required":["tool_name","params"],"additionalProperties":false}))
        })
        .collect();
    if let Some(alternatives) = schema
        .pointer_mut("/properties/tool_calls/items/anyOf")
        .and_then(Value::as_array_mut)
    {
        if targets.is_empty() {
            alternatives.retain(|v| {
                v.pointer("/properties/name/const").and_then(Value::as_str) != Some(ASK)
            });
        } else {
            for alternative in alternatives {
                if alternative
                    .pointer("/properties/name/const")
                    .and_then(Value::as_str)
                    == Some(ASK)
                {
                    alternative["properties"]["arguments"] = json!({"anyOf":targets});
                }
            }
        }
    }
    if schema
        .pointer("/properties/tool_calls/items/anyOf")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        // An ask-only/no-installed-tool sample must still permit an empty
        // finish; do not send an unsupported empty anyOf to the decoder.
        schema["properties"]["tool_calls"] = json!({"type":"array","maxItems":0});
    }
    schema
}

/// Check before SDK execution too; do not trust a decoder or model to enforce
/// the generation schema. Invalid proposals become observations, never calls.
pub fn rejection(call: &ToolCall, tools: &[ToolSpec]) -> Option<Value> {
    if call.name != ASK {
        return None;
    }
    let invalid = || {
        Some(json!({"ok":false,"stage":"clarification",
        "error":"Clarification target must be an available non-system tool; params must be a nonempty unique list of its exact schema field names. Recheck context and available lookups before asking."}))
    };
    let Some(args) = call.arguments.as_object() else {
        return invalid();
    };
    if args.len() != 2 {
        return invalid();
    }
    let Some(target) = args.get("tool_name").and_then(Value::as_str) else {
        return invalid();
    };
    let Some(tool) = tools.iter().find(|t| t.name == target && t.name != ASK) else {
        return invalid();
    };
    let Some(properties) = tool.parameters.get("properties").and_then(Value::as_object) else {
        return invalid();
    };
    let Some(params) = args.get("params").and_then(Value::as_array) else {
        return invalid();
    };
    if params.is_empty() {
        return invalid();
    }
    let mut seen = BTreeSet::new();
    for param in params {
        let Some(name) = param.as_str() else {
            return invalid();
        };
        if !properties.contains_key(name) || !seen.insert(name) {
            return invalid();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tools() -> Vec<ToolSpec> {
        [
            ("toy.write", json!({"type":"object","properties":{"title":{"type":"string"},"date":{"type":"string"}},"required":["title"]})),
            ("toy.list", json!({"type":"object","properties":{}})),
            (ASK, json!({"type":"object","properties":{"tool_name":{"type":"string"},"params":{"type":"array","items":{"type":"string"}}},"required":["tool_name","params"]}))
        ].into_iter().map(|(name, parameters)| ToolSpec {name:name.into(),parameters,description:String::new(),kind:"write".into(),returns:Value::Null}).collect()
    }
    fn ask(target: &str, params: Value) -> ToolCall {
        ToolCall {
            name: ASK.into(),
            arguments: json!({"tool_name":target,"params":params}),
        }
    }
    #[test]
    fn rejects_invented_disallowed_recursive_and_malformed_clarification() {
        let tools = tools();
        for call in [
            ask("toy.write", json!(["weather_condition"])),
            ask("other.write", json!(["title"])),
            ask(ASK, json!(["tool_name"])),
            ask("toy.list", json!(["anything"])),
            ask("toy.write", json!([])),
            ask("toy.write", json!(["title", "title"])),
            ask("toy.write", json!([1])),
        ] {
            assert!(rejection(&call, &tools).is_some());
        }
        assert!(
            rejection(
                &ToolCall {
                    name: ASK.into(),
                    arguments: json!({"tool_name":"toy.write","params":["title"],"extra":true})
                },
                &tools
            )
            .is_some()
        );
    }
    #[test]
    fn preserves_optional_fields_and_ordinary_calls_without_claiming_intent() {
        let tools = tools();
        assert!(rejection(&ask("toy.write", json!(["title", "date"])), &tools).is_none());
        assert!(
            rejection(
                &ToolCall {
                    name: "toy.write".into(),
                    arguments: json!({"title":"data"})
                },
                &tools
            )
            .is_none()
        );
    }
    #[test]
    fn decoder_only_changes_ask_and_does_not_mutate_sdk_or_enlarge_permissions() {
        let tools = tools();
        let before = serde_json::to_value(&tools).unwrap();
        let ordinary = crate::protocol::decision_schema(&tools);
        let bounded = decision_schema(&tools);
        assert_eq!(
            ordinary.pointer("/properties/tool_calls/items/anyOf/0"),
            bounded.pointer("/properties/tool_calls/items/anyOf/0")
        );
        let variants = bounded
            .pointer("/properties/tool_calls/items/anyOf/2/properties/arguments/anyOf")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0]["properties"]["tool_name"]["const"], "toy.write");
        let choices = variants[0]["properties"]["params"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(choices.len(), 4);
        for choice in [
            json!(["date"]),
            json!(["title"]),
            json!(["date", "title"]),
            json!(["title", "date"]),
        ] {
            assert!(choices.contains(&choice));
        }
        assert_eq!(serde_json::to_value(&tools).unwrap(), before);
    }
    #[test]
    fn ask_only_catalog_can_finish_without_an_unsatisfiable_item_schema() {
        let ask = tools().pop().unwrap();
        assert_eq!(
            decision_schema(&[ask])["properties"]["tool_calls"],
            json!({"type":"array","maxItems":0})
        );
    }
    #[test]
    fn small_schema_enumeration_preserves_every_unique_order_and_never_duplicates() {
        let mut budget = ENUM_BYTE_BUDGET;
        let names = ["a", "b", "c", "d", "e", "f"];
        let schema = params_schema(&names, &mut budget);
        let choices = schema["enum"].as_array().unwrap();
        assert_eq!(choices.len(), 1956); // sum P(6,k), k=1..6
        let mut distinct = BTreeSet::new();
        for choice in choices {
            let fields = choice.as_array().unwrap();
            assert!(!fields.is_empty() && fields.len() <= names.len());
            assert_eq!(
                fields
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<BTreeSet<_>>()
                    .len(),
                fields.len()
            );
            assert!(distinct.insert(choice.to_string()));
        }
        assert!(choices.contains(&json!(["f", "e", "d", "c", "b", "a"])));
        assert!(budget < ENUM_BYTE_BUDGET);
    }
    #[test]
    fn large_or_budget_exhausted_schema_stays_bounded_without_removing_valid_fields() {
        for names in [&["a", "b", "c", "d", "e", "f", "g"][..], &["a", "b"][..]] {
            let mut budget = 0;
            let schema = params_schema(names, &mut budget);
            assert!(schema.get("enum").is_none());
            assert_eq!(schema["maxItems"], names.len());
            assert_eq!(schema["items"]["enum"], json!(names));
        }
    }
}
