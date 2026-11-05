//! Progressive schema disclosure over an immutable, fully indexed catalog.
//! Inspection is internal metadata access only: it never invokes the SDK or
//! changes application state, and it never changes the authoritative schemas.

use crate::{MAX_JSON_BYTES, MAX_TOOLS, Result, grounding::model_tools, protocol::ToolSpec};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const META_TOOL: &str = "nora.inspect_tools";
const MAX_NAMES: usize = 12;
const MAX_INSPECTIONS: usize = 12;

/// Retains every supplied tool in the index while disclosing schemas on demand.
/// Successful inspection requests, including repeated requests, consume budget.
/// Rejected requests neither consume budget nor disclose any tools.
pub struct CatalogDisclosure {
    originals: Vec<ToolSpec>,
    names: BTreeSet<String>,
    index: String,
    meta: ToolSpec,
    seen: BTreeSet<String>,
    inspections: usize,
}

impl CatalogDisclosure {
    pub fn new(tools: &[ToolSpec]) -> Result<Self> {
        if tools.len() > MAX_TOOLS {
            return Err("progressive catalog exceeds 256 tools".into());
        }
        let mut names = BTreeSet::new();
        for tool in tools {
            if tool.name.trim().is_empty() {
                return Err("progressive catalog tool names must not be empty".into());
            }
            if tool.name == META_TOOL {
                return Err("catalog collides with internal inspection tool".into());
            }
            if !names.insert(tool.name.clone()) {
                return Err("duplicate tool name in progressive catalog".into());
            }
        }
        let index = serde_json::to_string(
            &tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "kind": tool.kind,
                        "description": tool.description,
                    })
                })
                .collect::<Vec<_>>(),
        )?;
        if index.len() > MAX_JSON_BYTES {
            return Err("progressive catalog index exceeds JSON byte limit".into());
        }
        let allowed_names: Vec<_> = tools.iter().map(|tool| tool.name.as_str()).collect();
        let meta = ToolSpec {
            name: META_TOOL.into(),
            description: "Internal catalog inspection only (internal_only; no_sdk). Return prompt-safe schemas for named tools from the complete index; no application action is executed. Inspect 1 to 12 unique names per request, at most 12 successful requests.".into(),
            kind: "read".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "names": {
                        "type": "array",
                        "items": {"type": "string", "enum": allowed_names},
                        "uniqueItems": true,
                        "minItems": 1,
                        "maxItems": MAX_NAMES,
                    }
                },
                "required": ["names"],
                "additionalProperties": false,
            }),
            returns: json!({
                "type": "object",
                "properties": {"tools": {"type": "array", "items": {"type": "object"}}},
                "required": ["tools"],
                "additionalProperties": false,
            }),
        };
        Ok(Self {
            originals: tools.to_vec(),
            names,
            index,
            meta,
            seen: BTreeSet::new(),
            inspections: 0,
        })
    }

    /// Complete name/kind/description index, unchanged by inspection or budget.
    pub fn index(&self) -> &str {
        &self.index
    }

    /// Internal inspection tool first, then seen tools in original catalog order.
    /// Only model-facing copies pass through example stripping.
    pub fn disclosed_tools(&self) -> Vec<ToolSpec> {
        let mut tools = vec![self.meta.clone()];
        tools.extend(
            self.originals
                .iter()
                .filter(|tool| self.seen.contains(&tool.name))
                .cloned(),
        );
        model_tools(&tools)
    }

    /// Validate the complete request before disclosing schemas or using budget.
    /// The returned metadata must not be sent to the SDK as an executed action.
    pub fn inspect(&mut self, arguments: &Value) -> Result<Value> {
        let object = arguments
            .as_object()
            .ok_or("inspection arguments must be an object")?;
        if object.len() != 1 || !object.contains_key("names") {
            return Err("inspection accepts exactly the names field".into());
        }
        let requested = object["names"]
            .as_array()
            .ok_or("inspection names must be an array")?;
        if requested.is_empty() || requested.len() > MAX_NAMES {
            return Err("inspection requires 1 to 12 names".into());
        }
        let mut selected = BTreeSet::new();
        for value in requested {
            let name = value.as_str().ok_or("inspection names must be strings")?;
            if !selected.insert(name) {
                return Err("inspection names must be unique".into());
            }
            if !self.names.contains(name) {
                return Err("inspection requested an unknown tool".into());
            }
        }
        if self.inspections >= MAX_INSPECTIONS {
            return Err("internal catalog inspection budget exhausted".into());
        }
        let tools: Vec<_> = self
            .originals
            .iter()
            .filter(|tool| selected.contains(tool.name.as_str()))
            .cloned()
            .collect();
        let result = json!({"tools": serde_json::to_value(model_tools(&tools))?});
        if serde_json::to_vec(&result)?.len() > MAX_JSON_BYTES {
            return Err("inspection result exceeds JSON byte limit".into());
        }
        self.seen.extend(selected.into_iter().map(str::to_owned));
        self.inspections += 1;
        Ok(result)
    }

    pub fn is_disclosed(&self, name: &str) -> bool {
        name == META_TOOL || self.seen.contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: format!("Description of {name}"),
            kind: "read".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "x-ref": "app.items",
                        "description": "Observed ID, ví dụ item_01",
                        "example": "item_01",
                        "examples": ["item_02"],
                    }
                },
                "required": ["id"],
                "additionalProperties": false,
                "examples": [{"id": "item_01"}],
            }),
            returns: json!({"type": "object"}),
        }
    }

    #[test]
    fn full_index_is_available_with_only_meta_schema_disclosed() {
        let tools = vec![tool("app.z"), tool("app.a")];
        let disclosure = CatalogDisclosure::new(&tools).unwrap();
        let index: Value = serde_json::from_str(disclosure.index()).unwrap();
        assert_eq!(
            index,
            json!([
                {"name": "app.z", "kind": "read", "description": "Description of app.z"},
                {"name": "app.a", "kind": "read", "description": "Description of app.a"},
            ])
        );
        let disclosed = disclosure.disclosed_tools();
        assert_eq!(disclosed.len(), 1);
        assert_eq!(disclosed[0].name, META_TOOL);
        assert!(disclosure.is_disclosed(META_TOOL));
        assert!(!disclosure.is_disclosed("app.z"));
        assert!(!disclosure.is_disclosed("missing.tool"));
        assert_eq!(
            disclosed[0].parameters,
            json!({
                "type": "object",
                "properties": {"names": {
                    "type": "array", "items": {"type": "string", "enum": ["app.z", "app.a"]},
                    "uniqueItems": true, "minItems": 1, "maxItems": 12
                }},
                "required": ["names"], "additionalProperties": false
            })
        );
        assert!(disclosed[0].description.contains("no_sdk"));
    }

    #[test]
    fn expansion_keeps_original_order_and_only_strips_prompt_copies() {
        let tools = vec![tool("app.z"), tool("app.b"), tool("app.a")];
        let original = serde_json::to_value(&tools).unwrap();
        let mut disclosure = CatalogDisclosure::new(&tools).unwrap();
        let index = disclosure.index().to_owned();
        let response = disclosure
            .inspect(&json!({"names": ["app.a", "app.z"]}))
            .unwrap();
        assert_eq!(response["tools"][0]["name"], "app.z");
        assert_eq!(response["tools"][1]["name"], "app.a");
        assert!(response["tools"][0]["parameters"].get("examples").is_none());
        let id = &response["tools"][0]["parameters"]["properties"]["id"];
        assert!(id.get("example").is_none());
        assert!(id.get("examples").is_none());
        assert_eq!(id["description"], "Observed ID");
        assert_eq!(id["x-ref"], "app.items");
        disclosure.inspect(&json!({"names": ["app.b"]})).unwrap();
        let names: Vec<_> = disclosure
            .disclosed_tools()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, [META_TOOL, "app.z", "app.b", "app.a"]);
        assert_eq!(disclosure.index(), index);
        assert_eq!(serde_json::to_value(&tools).unwrap(), original);
        assert_eq!(
            serde_json::to_value(&disclosure.originals).unwrap(),
            original
        );
    }

    #[test]
    fn invalid_requests_are_atomic_and_do_not_consume_budget() {
        let mut disclosure = CatalogDisclosure::new(&[tool("app.a"), tool("app.b")]).unwrap();
        disclosure.inspect(&json!({"names": ["app.a"]})).unwrap();
        let before = serde_json::to_value(disclosure.disclosed_tools()).unwrap();
        let invalid = [
            Value::Null,
            json!([]),
            json!({}),
            json!({"names": ["app.b"], "extra": true}),
            json!({"names": null}),
            json!({"names": "app.b"}),
            json!({"names": []}),
            json!({"names": ["app.b", 1]}),
            json!({"names": ["app.b", "app.b"]}),
            json!({"names": ["app.b", "unknown.tool"]}),
            json!({"names": [META_TOOL]}),
            json!({"names": vec!["app.b"; 13]}),
        ];
        for arguments in invalid {
            assert!(disclosure.inspect(&arguments).is_err(), "{arguments}");
            assert_eq!(disclosure.inspections, 1);
            assert!(!disclosure.is_disclosed("app.b"));
            assert_eq!(
                serde_json::to_value(disclosure.disclosed_tools()).unwrap(),
                before
            );
        }
        disclosure.inspect(&json!({"names": ["app.b"]})).unwrap();
        assert!(disclosure.is_disclosed("app.b"));
    }

    #[test]
    fn twelve_successful_inspections_are_allowed_and_thirteenth_is_atomic() {
        let mut disclosure = CatalogDisclosure::new(&[tool("app.a"), tool("app.b")]).unwrap();
        let index = disclosure.index().to_owned();
        for _ in 0..12 {
            disclosure.inspect(&json!({"names": ["app.a"]})).unwrap();
        }
        assert!(disclosure.inspect(&json!({"names": ["app.b"]})).is_err());
        assert_eq!(disclosure.inspections, 12);
        assert!(!disclosure.is_disclosed("app.b"));
        assert!(disclosure.is_disclosed("app.a"));
        assert_eq!(disclosure.index(), index);
        let names: Vec<_> = disclosure
            .disclosed_tools()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, [META_TOOL, "app.a"]);
    }

    #[test]
    fn ambiguous_reserved_and_oversized_catalogs_are_rejected() {
        assert!(CatalogDisclosure::new(&[tool("")]).is_err());
        assert!(CatalogDisclosure::new(&[tool("  ")]).is_err());
        assert!(CatalogDisclosure::new(&[tool("app.a"), tool("app.a")]).is_err());
        assert!(CatalogDisclosure::new(&[tool(META_TOOL)]).is_err());
        let tools: Vec<_> = (0..=256).map(|i| tool(&format!("app.tool_{i}"))).collect();
        assert!(CatalogDisclosure::new(&tools).is_err());
        assert!(CatalogDisclosure::new(&tools[..256]).is_ok());
    }

    #[test]
    fn oversized_index_and_inspection_results_are_rejected_before_mutation() {
        let mut large_index = tool("app.large_index");
        large_index.description = "x".repeat(MAX_JSON_BYTES);
        assert!(CatalogDisclosure::new(&[large_index]).is_err());

        let mut large_schema = tool("app.large_schema");
        large_schema.parameters = json!({"description": "x".repeat(MAX_JSON_BYTES)});
        let mut disclosure = CatalogDisclosure::new(&[large_schema, tool("app.small")]).unwrap();
        let index = disclosure.index().to_owned();
        assert!(
            disclosure
                .inspect(&json!({"names": ["app.small", "app.large_schema"]}))
                .is_err()
        );
        assert_eq!(disclosure.inspections, 0);
        assert!(!disclosure.is_disclosed("app.large_schema"));
        assert!(!disclosure.is_disclosed("app.small"));
        assert_eq!(disclosure.disclosed_tools().len(), 1);
        assert_eq!(disclosure.index(), index);
        for _ in 0..12 {
            disclosure
                .inspect(&json!({"names": ["app.small"]}))
                .unwrap();
        }
        assert!(
            disclosure
                .inspect(&json!({"names": ["app.small"]}))
                .is_err()
        );
    }
}
