//! Training-only target representation. This does not select inference actions.
//! Replace a labelled ID only when one prior successful observation contains it
//! in the explicit collection and application named by that parameter's x-ref.
use crate::protocol::{ToolCall, ToolSpec};
use serde_json::{Value, json};

/// Return an equivalent target; ambiguous/missing/unsupported sources stay literal.
/// Observations use the runtime Evidence shape: call, executed, result.
/// The caller supplies ONLY the current causal prefix, never future gold results.
pub fn bind_observed_ids(
    labelled: &ToolCall,
    observations: &[Value],
    tools: &[ToolSpec],
) -> ToolCall {
    let mut target = labelled.clone();
    let Some(tool) = tools.iter().find(|t| t.name == labelled.name) else {
        return target;
    };
    let Some(arguments) = target.arguments.as_object_mut() else {
        return target;
    };
    for (parameter, value) in arguments {
        let Some(id) = value.as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some((owner, collection)) = tool.parameters["properties"][parameter]["x-ref"]
            .as_str()
            .and_then(|s| s.split_once('.'))
        else {
            continue;
        };
        let mut candidates = Vec::new();
        for (index, observation) in observations.iter().enumerate() {
            if observation["executed"] != true
                || observation["result"]["ok"] != true
                || observation["result"]["done"] == false
                || observation["call"]["name"]
                    .as_str()
                    .and_then(|s| s.split_once('.'))
                    .is_none_or(|(app, _)| app != owner)
            {
                continue;
            }
            let Some(rows) = observation["result"][collection].as_array() else {
                continue;
            };
            for (row_index, row) in rows.iter().enumerate() {
                if row["id"].as_str() == Some(id) {
                    let escaped = collection.replace('~', "~0").replace('/', "~1");
                    candidates.push(format!(
                        "/observations/{index}/result/{escaped}/{row_index}/id"
                    ));
                }
            }
        }
        if candidates.len() == 1 {
            *value = json!({"$source":candidates[0]});
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{actor_v2::Evidence, bridge::CallOutcome};

    fn tool(reference: &str) -> ToolSpec {
        ToolSpec {
            name: "calendar.update".into(),
            description: String::new(),
            kind: "write".into(),
            parameters: json!({"properties":{"event_id":{"x-ref":reference},"title":{"type":"string"}}}),
            returns: Value::Null,
        }
    }
    fn call() -> ToolCall {
        ToolCall {
            name: "calendar.update".into(),
            arguments: json!({"event_id":"event-2","title":"event-2"}),
        }
    }
    fn observation(app: &str, collection: &str) -> Value {
        json!({"call":{"name":format!("{app}.list"),"arguments":{}},"executed":true,
            "result":{"ok":true,collection:[{"id":"event-1"},{"id":"event-2"}]}})
    }
    #[test]
    fn source_resolves_through_real_runtime_and_non_reference_values_stay_literal() {
        let tools = vec![tool("calendar.events")];
        let row = observation("calendar", "events");
        let labelled = call();
        let proposed = bind_observed_ids(&labelled, std::slice::from_ref(&row), &tools);
        assert_eq!(
            proposed.arguments["event_id"],
            json!({"$source":"/observations/0/result/events/1/id"})
        );
        assert_eq!(proposed.arguments["title"], "event-2");
        let mut evidence = Evidence::begin(&json!({}), &[], &tools);
        let read = serde_json::from_value(row["call"].clone()).unwrap();
        evidence.record(
            &read,
            &CallOutcome {
                executed: true,
                validation: json!({"ok":true}),
                result: Some(row["result"].clone()),
            },
            &tools,
        );
        assert_eq!(evidence.bind(&proposed).unwrap(), labelled);
    }
    #[test]
    fn wrong_namespaces_ambiguous_failed_and_absent_observations_cannot_bind() {
        let tools = vec![tool("calendar.events")];
        let labelled = call();
        let correct = observation("calendar", "events");
        let mut failed = correct.clone();
        failed["result"]["ok"] = json!(false);
        let mut unexecuted = correct.clone();
        unexecuted["executed"] = json!(false);
        let mut incomplete = correct.clone();
        incomplete["result"]["done"] = json!(false);
        for rows in [
            vec![],
            vec![observation("contacts", "events")],
            vec![observation("calendar", "tasks")],
            vec![correct.clone(), correct],
            vec![failed],
            vec![unexecuted],
            vec![incomplete],
        ] {
            assert_eq!(bind_observed_ids(&labelled, &rows, &tools), labelled);
        }
    }
    #[test]
    fn collection_pointer_escaping_does_not_reinterpret_paths() {
        let tools = vec![tool("calendar.events~/archive")];
        let target = bind_observed_ids(
            &call(),
            &[observation("calendar", "events~/archive")],
            &tools,
        );
        assert_eq!(
            target.arguments["event_id"]["$source"],
            "/observations/0/result/events~0~1archive/1/id"
        );
    }
}
