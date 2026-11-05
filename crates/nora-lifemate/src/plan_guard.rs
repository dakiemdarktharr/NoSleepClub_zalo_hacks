//! Pre-fragment literal provenance checks for an already validated Plan.
//!
//! Run before any effect, against provenance captured before this fragment.
//! This prevents a guessed literal from becoming authorized retroactively by
//! an earlier read in the same generated plan. Membership is not user intent,
//! SDK schema validation, or proof that a reference selects the right entity.
use crate::{
    grounding::{IdProvenance, normalize_call},
    plan::{Expr, Plan, Step},
    protocol::{ToolCall, ToolSpec},
};
use serde_json::{Value, json};

/// Return the first literal rejection, preserving the existing validation.
/// Call `planning::parse_plan` / `Plan::validate` first. Dynamic refs remain
/// subject to executor source checks and runtime grounding of resolved calls.
pub fn literal_rejection(
    plan: &Plan,
    provenance: &IdProvenance,
    tools: &[ToolSpec],
) -> Option<Value> {
    for step in &plan.steps {
        let Step::Call {
            id,
            tool,
            arguments,
            ..
        } = step
        else {
            continue;
        };
        let literals = arguments
            .iter()
            .filter_map(|(name, expression)| match expression {
                Expr::Literal { value } => Some((name.clone(), value.clone())),
                // Object/Array expressions preserve their outer JSON type;
                // they cannot evaluate to a scalar ID/phone string. Original
                // SDK validation still decides whether that type is legal.
                Expr::Ref { .. } | Expr::Object { .. } | Expr::Array { .. } => None,
            })
            .collect();
        let proposal = ToolCall {
            name: tool.clone(),
            arguments: Value::Object(literals),
        };
        let (call, _) = normalize_call(&proposal, tools);
        if let Some(validation) = provenance.rejection(&call, tools) {
            return Some(json!({"step":id,"tool":tool,"validation":validation}));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bridge::CallOutcome,
        plan::{ErrorKind, execute},
    };
    use std::time::{Duration, Instant};

    fn tools() -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "contacts.search".into(),
                kind: "read".into(),
                description: "Find contacts".into(),
                parameters: json!({"type":"object","properties":{"query":{"type":"string"}}}),
                returns: Value::Null,
            },
            ToolSpec {
                name: "phone.call".into(),
                kind: "write".into(),
                description: "Call the selected contact or number".into(),
                parameters: json!({"type":"object","properties":{
                    "contact_id":{"type":"string","x-ref":"contacts.contacts"},
                    "phone_number":{"type":"string"}},"additionalProperties":false}),
                returns: Value::Null,
            },
        ]
    }

    fn parsed(value: Value, tools: &[ToolSpec]) -> Plan {
        let plan: Plan = serde_json::from_value(value).unwrap();
        plan.validate(tools).unwrap();
        plan
    }

    #[test]
    fn guessed_literal_is_rejected_before_a_read_can_retroactively_authorize_it() {
        let tools = tools();
        let plan = parsed(
            json!({"steps":[
                {"op":"call","id":"lookup","tool":"contacts.search","arguments":{}},
                {"op":"call","id":"dial","tool":"phone.call","arguments":{
                    "contact_id":{"kind":"literal","value":"person_guessed"}}}
            ]}),
            &tools,
        );
        let mut provenance = IdProvenance::default();
        let rejection = literal_rejection(&plan, &provenance, &tools).unwrap();
        assert_eq!(rejection["step"], "dial");
        assert_eq!(rejection["tool"], "phone.call");
        assert_eq!(rejection["validation"]["stage"], "provenance");
        assert_eq!(rejection["validation"]["parameter"], "contact_id");
        assert_eq!(rejection["validation"]["error"], "unobserved_reference");
        // Checking after the read would lose this distinction, hence the
        // caller must run the guard before executing any plan step.
        provenance.observe(&json!({"ok":true,"contacts":[{"id":"person_guessed"}]}));
        assert!(literal_rejection(&plan, &provenance, &tools).is_none());
    }

    #[test]
    fn ids_observed_before_the_fragment_are_allowed() {
        let tools = tools();
        let plan = parsed(
            json!({"steps":[{"op":"call","id":"dial","tool":"phone.call","arguments":{
                "contact_id":{"kind":"literal","value":"person_known"}}}]}),
            &tools,
        );
        let provenance = IdProvenance::begin(&json!({"contact":{"id":"person_known"}}), &[], "");
        assert!(literal_rejection(&plan, &provenance, &tools).is_none());
    }

    #[test]
    fn a_dynamic_ref_requires_a_successful_completed_actual_source() {
        let tools = tools();
        let plan = parsed(
            json!({"steps":[
                {"op":"call","id":"lookup","tool":"contacts.search","arguments":{}},
                {"op":"call","id":"dial","tool":"phone.call","arguments":{
                    "contact_id":{"kind":"ref","step":"lookup","pointer":"/contacts/0/id"}}}
            ]}),
            &tools,
        );
        for (result, expected_error, expected_calls) in [
            (json!({"ok":true,"contacts":[{"id":"person_new"}]}), None, 2),
            (
                json!({"ok":false,"contacts":[{"id":"person_new"}]}),
                Some(ErrorKind::CallFailed),
                1,
            ),
            (
                json!({"ok":true,"done":false,"contacts":[{"id":"person_new"}]}),
                Some(ErrorKind::CallIncomplete),
                1,
            ),
        ] {
            let mut provenance = IdProvenance::default();
            assert!(literal_rejection(&plan, &provenance, &tools).is_none());
            let mut calls = 0;
            let report = execute(
                &plan,
                &tools,
                Instant::now() + Duration::from_secs(1),
                |call, _| {
                    calls += 1;
                    assert!(provenance.rejection(call, &tools).is_none());
                    let result = if call.name == "contacts.search" {
                        result.clone()
                    } else {
                        assert_eq!(call.arguments, json!({"contact_id":"person_new"}));
                        json!({"ok":true})
                    };
                    if result["ok"] == true && result.get("done") != Some(&Value::Bool(false)) {
                        provenance.observe(&result);
                    }
                    Ok(CallOutcome {
                        executed: true,
                        validation: json!({"ok":true}),
                        result: Some(result),
                    })
                },
            );
            assert_eq!(calls, expected_calls);
            assert_eq!(report.error.map(|error| error.kind), expected_error);
            assert_eq!(report.completed, expected_error.is_none());
        }
    }

    #[test]
    fn literal_phones_must_be_observed_and_the_original_validation_is_retained() {
        let tools = tools();
        let plan = parsed(
            json!({"steps":[{"op":"call","id":"dial","tool":"phone.call","arguments":{
                "phone_number":{"kind":"literal","value":"+84912345678"}}}]}),
            &tools,
        );
        let before = IdProvenance::default();
        let rejection = literal_rejection(&plan, &before, &tools).unwrap();
        let actual = before
            .rejection(
                &ToolCall {
                    name: "phone.call".into(),
                    arguments: json!({"phone_number":"+84912345678"}),
                },
                &tools,
            )
            .unwrap();
        assert_eq!(rejection["validation"], actual);
        let spoken = IdProvenance::begin(&json!({}), &[], "Gọi +84912345678");
        assert!(literal_rejection(&plan, &spoken, &tools).is_none());
    }

    #[test]
    fn composite_expressions_are_not_coerced_into_id_strings() {
        let tools = tools();
        for expression in [
            json!({"kind":"object","fields":{"id":{"kind":"literal","value":"person_guessed"}}}),
            json!({"kind":"array","items":[{"kind":"literal","value":"person_guessed"}]}),
        ] {
            let plan = parsed(
                json!({"steps":[{"op":"call","id":"dial","tool":"phone.call","arguments":{
                    "contact_id":expression}}]}),
                &tools,
            );
            assert!(literal_rejection(&plan, &IdProvenance::default(), &tools).is_none());
            let report = execute(
                &plan,
                &tools,
                Instant::now() + Duration::from_secs(1),
                |call, _| {
                    // The guard does not approve SDK types. These values keep
                    // their non-string type and are refused by this fixture's
                    // original string parameter contract.
                    assert!(call.arguments["contact_id"].as_str().is_none());
                    Ok(CallOutcome {
                        executed: false,
                        validation: json!({"ok":false,"stage":"schema"}),
                        result: None,
                    })
                },
            );
            assert_eq!(report.error.unwrap().kind, ErrorKind::CallRejected);
            assert!(!report.completed);
        }
    }
}
