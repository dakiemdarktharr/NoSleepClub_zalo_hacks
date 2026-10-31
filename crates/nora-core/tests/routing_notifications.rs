use nora_core::from_json_bounded;
use nora_core::notifications::{Context, DeliveryState, Event, EventKind, Speech, Surface, decide};
use nora_core::routing::{Gate, Mode, TaskFacts, route};
use serde_json::json;

fn event(kind: EventKind) -> Event {
    Event {
        goal_id: "g".into(),
        event_id: "e".into(),
        kind,
        occurred_at: 100.0,
        summary: "Một phát hiện có ích.".into(),
        milestone_key: String::new(),
        verified: true,
        evidence_refs: vec!["receipt:1".into()],
        read_source_ids: Vec::new(),
    }
}

#[test]
fn risk_gate_is_independent_of_short_task_complexity() {
    let result = route(&TaskFacts {
        requires_tools: true,
        expected_steps: 1,
        needs_effect_approval: true,
        ..TaskFacts::default()
    })
    .unwrap();
    assert_eq!((result.mode, result.gate), (Mode::Tool, Gate::Approval));
}

#[test]
fn missing_permission_preserves_durable_goal_and_input_precedes_approval() {
    let mut facts = TaskFacts {
        requires_tools: true,
        has_dependencies: true,
        requires_resume: true,
        needs_new_grant: true,
        needs_effect_approval: true,
        ..TaskFacts::default()
    };
    let result = route(&facts).unwrap();
    assert_eq!((result.mode, result.gate), (Mode::Goal, Gate::Permission));
    facts.critical_slot_ambiguous = true;
    assert_eq!(route(&facts).unwrap().gate, Gate::Input);
}

#[test]
fn unsupported_tools_never_become_a_goal_to_install_something() {
    let result = route(&TaskFacts {
        requires_tools: true,
        tool_supported: false,
        requires_resume: true,
        ..TaskFacts::default()
    })
    .unwrap();
    assert_eq!(result.mode, Mode::Unsupported);
    assert_eq!(result.reason, "no_supported_tool");
}

#[test]
fn strict_json_rejects_string_booleans_negative_or_fractional_steps_and_unknown_fields() {
    for bad in [
        json!({"requires_tools":"false"}),
        json!({"expected_steps":true}),
        json!({"expected_steps":-1}),
        json!({"expected_steps":0.5}),
        json!({"needs_effect_aproval":true}),
    ] {
        assert!(serde_json::from_value::<TaskFacts>(bad).is_err());
    }
    let facts: TaskFacts = serde_json::from_value(json!({})).unwrap();
    assert_eq!(route(&facts).unwrap().mode, Mode::Answer);
    assert!(
        route(&TaskFacts {
            missing_parameters: vec![" ".into()],
            ..facts
        })
        .is_err()
    );
}

#[test]
fn diagnostic_events_and_unverified_success_are_silent() {
    for kind in [EventKind::Technical, EventKind::Heartbeat] {
        let result = decide(&event(kind), &Context::new(100.0), DeliveryState::default()).unwrap();
        assert_eq!(
            (result.surface, result.speech),
            (Surface::None, Speech::None)
        );
        assert_eq!(result.reason, "diagnostic_only");
    }
    let mut completion = event(EventKind::GoalCompleted);
    completion.verified = false;
    assert_eq!(
        decide(&completion, &Context::new(100.0), DeliveryState::default())
            .unwrap()
            .reason,
        "unverified_claim"
    );
    completion.verified = true;
    completion.evidence_refs.clear();
    assert_eq!(
        decide(&completion, &Context::new(100.0), DeliveryState::default())
            .unwrap()
            .reason,
        "unverified_claim"
    );
}

#[test]
fn sources_count_unique_read_ids_not_model_claim_or_search_slots() {
    let mut milestone = event(EventKind::SourcesMilestone);
    milestone.summary = "Tôi đã đọc 999 nguồn!".into();
    milestone.read_source_ids = ["a", "b", "a", "c"].map(str::to_owned).to_vec();
    let result = decide(&milestone, &Context::new(100.0), DeliveryState::default()).unwrap();
    assert_eq!(
        result.text,
        "Đã đối chiếu 3 nguồn duy nhất có nội dung đã đọc."
    );
    milestone.read_source_ids = vec!["same".into(), "same".into()];
    assert_eq!(
        decide(&milestone, &Context::new(100.0), DeliveryState::default())
            .unwrap()
            .reason,
        "below_source_milestone"
    );
}

#[test]
fn quiet_mode_retains_action_card_and_status_is_spoken() {
    let context = Context {
        now: 100.0,
        quiet: true,
        ..Context::default()
    };
    let result = decide(
        &event(EventKind::PermissionRequired),
        &context,
        DeliveryState::default(),
    )
    .unwrap();
    assert_eq!(
        (result.surface, result.speech),
        (Surface::ActionCard, Speech::None)
    );
    assert_eq!(
        decide(
            &event(EventKind::StatusRequested),
            &context,
            DeliveryState::default()
        )
        .unwrap()
        .speech,
        Speech::Ready
    );
}

#[test]
fn interruption_defers_without_delivery_acknowledgment() {
    let completion = event(EventKind::GoalCompleted);
    let context = Context {
        now: 100.0,
        user_speaking: true,
        ..Context::default()
    };
    assert_eq!(
        decide(&completion, &context, DeliveryState::default())
            .unwrap()
            .speech,
        Speech::Defer
    );
    assert_eq!(
        decide(&completion, &Context::new(110.0), DeliveryState::default())
            .unwrap()
            .speech,
        Speech::Ready
    );
}

#[test]
fn routine_cooldown_does_not_suppress_urgent_blocker() {
    let state = DeliveryState {
        last_speech: Some(95.0),
        ..DeliveryState::default()
    };
    assert_eq!(
        decide(
            &event(EventKind::FindingMaterial),
            &Context::new(100.0),
            state
        )
        .unwrap()
        .speech,
        Speech::Defer
    );
    assert_eq!(
        decide(
            &event(EventKind::BlockerMaterial),
            &Context::new(100.0),
            state
        )
        .unwrap()
        .speech,
        Speech::Ready
    );
}

#[test]
fn channels_are_independent_and_notice_key_is_stable_across_event_retry() {
    let first = event(EventKind::FindingMaterial);
    let state = DeliveryState {
        surface_delivered: true,
        ..DeliveryState::default()
    };
    let result = decide(&first, &Context::new(100.0), state).unwrap();
    assert_eq!(
        (result.surface, result.speech),
        (Surface::None, Speech::Ready)
    );
    let state = DeliveryState {
        surface_delivered: true,
        speech_delivered: true,
        last_speech: Some(100.0),
    };
    assert_eq!(
        decide(&first, &Context::new(150.0), state).unwrap().speech,
        Speech::None
    );
    let mut retry = first.clone();
    retry.milestone_key = "finding-1".into();
    let key = retry.notice_key();
    retry.event_id = "new-event-id".into();
    assert_eq!(retry.notice_key(), key);
    retry.kind = EventKind::BlockerMaterial;
    assert_ne!(retry.notice_key(), key);
}

#[test]
fn nonfinite_times_bad_ids_and_oversize_messages_are_rejected() {
    let mut e = event(EventKind::GoalCompleted);
    for time in [f64::NAN, f64::INFINITY, -1.0] {
        e.occurred_at = time;
        assert!(decide(&e, &Context::new(100.0), DeliveryState::default()).is_err());
    }
    e.occurred_at = 100.0;
    e.goal_id = " ".into();
    assert!(decide(&e, &Context::new(100.0), DeliveryState::default()).is_err());
    assert!(from_json_bounded::<TaskFacts>(&vec![b' '; nora_core::MAX_JSON_BYTES + 1]).is_err());
    assert!(serde_json::from_value::<Context>(json!({"now":1,"quiet":"false"})).is_err());
    assert!(serde_json::from_value::<Event>(json!({"goal_id":"g","event_id":"e","kind":"raw.reasoning","occurred_at":1,"summary":"x"})).is_err());
    assert!(serde_json::from_value::<Event>(json!({"goal_id":"g","event_id":"e","kind":"goal.completed","occurred_at":1,"summary":"x","verified":"true"})).is_err());
}

#[test]
fn future_event_produces_reference_empty_decision_shape() {
    let mut e = event(EventKind::FindingMaterial);
    e.occurred_at = 101.0;
    assert_eq!(
        serde_json::to_value(decide(&e, &Context::new(100.0), DeliveryState::default()).unwrap())
            .unwrap(),
        json!({
            "surface":"none", "speech":"none", "reason":"future_event", "text":"", "notice_key":""
        })
    );
}

#[test]
fn checked_in_nine_event_fixture_retains_reference_decisions() {
    let fixture = include_str!("../../../examples/progress-events.jsonl");
    let expected = [
        ("timeline", "ready", "meaningful_event"),
        ("none", "none", "diagnostic_only"),
        ("none", "none", "diagnostic_only"),
        ("timeline", "ready", "meaningful_event"),
        ("timeline", "ready", "meaningful_event"),
        ("action_card", "ready", "meaningful_event"),
        ("none", "none", "unverified_claim"),
        ("timeline", "ready", "meaningful_event"),
        ("none", "none", "diagnostic_only"),
    ];
    let mut last_speech = None;
    for (line, expected) in fixture.lines().zip(expected) {
        let row: serde_json::Value = from_json_bounded(line.as_bytes()).unwrap();
        let event: Event = serde_json::from_value(row["event"].clone()).unwrap();
        let decision = decide(
            &event,
            &Context::new(event.occurred_at),
            DeliveryState {
                last_speech,
                ..DeliveryState::default()
            },
        )
        .unwrap();
        let value = serde_json::to_value(&decision).unwrap();
        assert_eq!(
            (&value["surface"], &value["speech"], &value["reason"]),
            (&json!(expected.0), &json!(expected.1), &json!(expected.2))
        );
        if decision.speech == Speech::Ready {
            last_speech = Some(event.occurred_at);
        }
    }
}
