use nora_core::policy::{
    ActionContext, ActionProposal, ApprovalBinding, CapabilityGrant, DataClass, EffectKind,
    FieldConstraint, FieldEvidence, TrustedDataUse, Verdict, action_fingerprint, assess_action,
    canonical_sha256,
};
use serde_json::json;

fn action() -> ActionProposal {
    ActionProposal {
        action_id: "action-1".into(),
        goal_id: "goal-1".into(),
        goal_version: 1,
        account: "merchant-1".into(),
        tool: "message.send".into(),
        effect: EffectKind::Network,
        canonical_target: "customer:42".into(),
        arguments: json!({"recipient":"customer:42","text":"Đơn của bạn đủ điều kiện đổi hàng."}),
    }
}

fn grant(action: &ActionProposal) -> CapabilityGrant {
    CapabilityGrant {
        id: "grant-1".into(),
        revision: 1,
        policy_revision: 1,
        goal_id: action.goal_id.clone(),
        goal_version: action.goal_version,
        account: action.account.clone(),
        tool: action.tool.clone(),
        effect: action.effect,
        canonical_target: action.canonical_target.clone(),
        expires_at_ms: 1000,
        revoked: false,
        requires_approval: false,
        max_data_class: DataClass::Personal,
        max_payload_bytes: 4096,
        allowed_destinations: vec![action.canonical_target.clone()],
        field_constraints: Vec::new(),
    }
}

fn allowed_data(target: &str) -> ActionContext {
    ActionContext {
        data: vec![
            TrustedDataUse::new(
                "order-record".into(),
                DataClass::Personal,
                vec![target.into()],
            )
            .unwrap(),
        ],
        ..ActionContext::default()
    }
}

#[test]
fn typed_scope_matches_exact_goal_account_tool_target_and_effect() {
    let original = action();
    let g = grant(&original);
    assert_eq!(
        assess_action(
            &original,
            &g,
            &allowed_data(&original.canonical_target),
            None,
            100
        )
        .unwrap()
        .verdict,
        Verdict::Allow
    );
    let mut variants = Vec::new();
    let mut x = original.clone();
    x.goal_version = 2;
    variants.push(x);
    let mut x = original.clone();
    x.goal_id = "other-goal".into();
    variants.push(x);
    let mut x = original.clone();
    x.account = "other-merchant".into();
    variants.push(x);
    let mut x = original.clone();
    x.tool = "data.export".into();
    variants.push(x);
    let mut x = original.clone();
    x.canonical_target = "external:attacker".into();
    variants.push(x);
    let mut x = original.clone();
    x.effect = EffectKind::LocalWrite;
    variants.push(x);
    for x in variants {
        let decision = assess_action(&x, &g, &ActionContext::default(), None, 100).unwrap();
        assert_eq!(decision.verdict, Verdict::Deny);
        assert_eq!(decision.reason, "outside_capability_scope");
    }
}

#[test]
fn source_destination_and_classification_gate_every_model_request() {
    let mut a = action();
    a.effect = EffectKind::ModelRequest;
    a.tool = "llm.request".into();
    a.canonical_target = "provider:configured".into();
    let mut g = grant(&a);
    let forbidden = ActionContext {
        data: vec![
            TrustedDataUse::new(
                "private-file".into(),
                DataClass::Personal,
                vec!["local:only".into()],
            )
            .unwrap(),
        ],
        ..ActionContext::default()
    };
    assert_eq!(
        assess_action(&a, &g, &forbidden, None, 100).unwrap().reason,
        "source_destination_denied"
    );
    let confidential = ActionContext {
        data: vec![
            TrustedDataUse::new(
                "private-file".into(),
                DataClass::Confidential,
                vec![a.canonical_target.clone()],
            )
            .unwrap(),
        ],
        ..ActionContext::default()
    };
    assert_eq!(
        assess_action(&a, &g, &confidential, None, 100)
            .unwrap()
            .reason,
        "classification_not_granted"
    );
    g.max_data_class = DataClass::Secret;
    let secret = ActionContext {
        data: vec![
            TrustedDataUse::new(
                "api-token".into(),
                DataClass::Secret,
                vec![a.canonical_target.clone()],
            )
            .unwrap(),
        ],
        ..ActionContext::default()
    };
    assert_eq!(
        assess_action(&a, &g, &secret, None, 100).unwrap().reason,
        "secret_egress_denied"
    );
}

#[test]
fn local_writes_also_obey_source_destination_and_data_class() {
    let mut a = action();
    a.effect = EffectKind::LocalWrite;
    let g = grant(&a);
    let context = ActionContext {
        data: vec![
            TrustedDataUse::new(
                "file".into(),
                DataClass::Personal,
                vec!["local:another".into()],
            )
            .unwrap(),
        ],
        ..ActionContext::default()
    };
    assert_eq!(
        assess_action(&a, &g, &context, None, 100).unwrap().reason,
        "source_destination_denied"
    );
}

#[test]
fn egress_without_trusted_provenance_and_nonobject_arguments_are_rejected() {
    let mut a = action();
    let g = grant(&a);
    assert_eq!(
        assess_action(&a, &g, &ActionContext::default(), None, 100)
            .unwrap()
            .reason,
        "missing_data_provenance"
    );
    a.arguments = json!("unstructured tool arguments");
    assert!(a.validate().is_err());
}

#[test]
fn critical_field_needs_trusted_value_and_matching_source_then_accepts_genuine_change() {
    let mut a = action();
    let mut g = grant(&a);
    g.field_constraints.push(FieldConstraint {
        json_pointer: "/recipient".into(),
        expected_value: json!("customer:42"),
        source_id: "order:123".into(),
    });
    assert_eq!(
        assess_action(&a, &g, &ActionContext::default(), None, 100)
            .unwrap()
            .verdict,
        Verdict::NeedsInput
    );
    let mut context = allowed_data(&a.canonical_target);
    context
        .field_evidence
        .push(FieldEvidence::new("/recipient".into(), "order:123".into()).unwrap());
    assert_eq!(
        assess_action(&a, &g, &context, None, 100).unwrap().verdict,
        Verdict::Allow
    );
    a.arguments["recipient"] = json!("customer:43");
    assert_eq!(
        assess_action(&a, &g, &context, None, 100).unwrap().verdict,
        Verdict::NeedsInput
    );
    // A newly authorized source updates the field contract, rather than a rule
    // that rejects every legitimate change or silently keeps the old value.
    g.revision += 1;
    g.field_constraints[0].expected_value = json!("customer:43");
    g.field_constraints[0].source_id = "user-confirmation:2".into();
    context.field_evidence[0] =
        FieldEvidence::new("/recipient".into(), "user-confirmation:2".into()).unwrap();
    assert_eq!(
        assess_action(&a, &g, &context, None, 100).unwrap().verdict,
        Verdict::Allow
    );
}

#[test]
fn approval_binds_action_content_id_scope_provenance_revisions_and_expiry() {
    let a = action();
    let mut g = grant(&a);
    g.requires_approval = true;
    let context = allowed_data(&a.canonical_target);
    assert_eq!(
        assess_action(&a, &g, &context, None, 100).unwrap().verdict,
        Verdict::NeedsApproval
    );
    let approval = ApprovalBinding::from_trusted_confirmation(&a, &g, &context, 50, 500).unwrap();
    assert_eq!(
        assess_action(&a, &g, &context, Some(&approval), 100)
            .unwrap()
            .verdict,
        Verdict::Allow
    );
    let mut changed = a.clone();
    changed.arguments["text"] = json!("Different attachment or message content");
    assert_eq!(
        assess_action(&changed, &g, &context, Some(&approval), 100)
            .unwrap()
            .verdict,
        Verdict::NeedsApproval
    );
    changed = a.clone();
    changed.action_id = "different-action-same-content".into();
    assert!(!approval.valid_for(&changed, &g, &context, 100).unwrap());
    let mut changed_g = g.clone();
    changed_g.revision += 1;
    assert!(!approval.valid_for(&a, &changed_g, &context, 100).unwrap());
    changed_g = g.clone();
    changed_g.policy_revision += 1;
    assert!(!approval.valid_for(&a, &changed_g, &context, 100).unwrap());
    let changed_context = ActionContext {
        data: vec![
            TrustedDataUse::new(
                "new-source".into(),
                DataClass::Personal,
                vec![a.canonical_target.clone()],
            )
            .unwrap(),
        ],
        ..ActionContext::default()
    };
    assert!(!approval.valid_for(&a, &g, &changed_context, 100).unwrap());
    assert!(!approval.valid_for(&a, &g, &context, 49).unwrap());
    assert!(!approval.valid_for(&a, &g, &context, 500).unwrap());
    assert_eq!(
        assess_action(&a, &g, &context, Some(&approval), 1000)
            .unwrap()
            .verdict,
        Verdict::Deny
    );
    g.revoked = true;
    assert_eq!(
        assess_action(&a, &g, &context, Some(&approval), 100)
            .unwrap()
            .verdict,
        Verdict::Deny
    );
}

#[test]
fn canonical_fingerprint_sorts_nested_object_keys_and_detects_material_changes() {
    let a: serde_json::Value =
        serde_json::from_str(r#"{"z":[{"b":2,"a":1}],"a":"tiếng Việt"}"#).unwrap();
    let b: serde_json::Value =
        serde_json::from_str(r#"{"a":"tiếng Việt","z":[{"a":1,"b":2}]}"#).unwrap();
    assert_eq!(canonical_sha256(&a).unwrap(), canonical_sha256(&b).unwrap());
    let mut request = action();
    let before = action_fingerprint(&request).unwrap();
    request.arguments["recipient"] = json!("another-customer");
    assert_ne!(before, action_fingerprint(&request).unwrap());
}

#[test]
fn delegation_can_narrow_but_never_expand_scope_or_remove_approval() {
    let a = action();
    let mut parent = grant(&a);
    parent.requires_approval = true;
    let mut child = parent.clone();
    child.id = "child".into();
    child.expires_at_ms = 500;
    child.max_payload_bytes = 512;
    assert!(child.is_subset_of(&parent).unwrap());
    child.requires_approval = false;
    assert!(!child.is_subset_of(&parent).unwrap());
    child.requires_approval = true;
    child.allowed_destinations.push("exfil:external".into());
    assert!(!child.is_subset_of(&parent).unwrap());
    child.allowed_destinations.pop();
    child.max_data_class = DataClass::Secret;
    assert!(!child.is_subset_of(&parent).unwrap());
    child.max_data_class = parent.max_data_class;
    child.canonical_target = "another-target".into();
    assert!(!child.is_subset_of(&parent).unwrap());
}

#[test]
fn oversized_or_deep_json_and_duplicate_provenance_are_rejected() {
    let huge = json!("x".repeat(nora_core::MAX_TEXT_BYTES + 1));
    assert!(canonical_sha256(&huge).is_err());
    let mut deep = json!(null);
    for _ in 0..40 {
        deep = json!([deep]);
    }
    assert!(canonical_sha256(&deep).is_err());
    let a = action();
    let mut g = grant(&a);
    g.max_payload_bytes = 1;
    assert_eq!(
        assess_action(&a, &g, &ActionContext::default(), None, 100)
            .unwrap()
            .reason,
        "payload_budget_exceeded"
    );
    let mut g = grant(&a);
    let field = FieldConstraint {
        json_pointer: "/recipient".into(),
        expected_value: json!("customer:42"),
        source_id: "order:123".into(),
    };
    g.field_constraints = vec![field.clone(), field];
    assert!(g.validate().is_err());
    let field = FieldEvidence::new("/recipient".into(), "order:123".into()).unwrap();
    assert!(
        ActionContext {
            field_evidence: vec![field.clone(), field],
            ..ActionContext::default()
        }
        .validate()
        .is_err()
    );
}
