//! Evidence-gated, channel-independent notification decisions.
//!
//! `verified` is retained for Python fixture/wire compatibility, not a proof or
//! authentication mechanism. Only authenticated broker/verifier code may supply
//! events to this module. Never deserialize model output and treat it as a
//! trusted runtime event. Delivery state is supplied by the durable ledger.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{CoreError, MAX_ID_BYTES, MAX_TEXT_BYTES, nonempty, refs, timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    #[serde(rename = "goal.accepted")]
    GoalAccepted,
    #[serde(rename = "worker.started")]
    WorkerStarted,
    #[serde(rename = "sources.milestone")]
    SourcesMilestone,
    #[serde(rename = "finding.material")]
    FindingMaterial,
    #[serde(rename = "permission.required")]
    PermissionRequired,
    #[serde(rename = "input.required")]
    InputRequired,
    #[serde(rename = "blocker.material")]
    BlockerMaterial,
    #[serde(rename = "goal.completed")]
    GoalCompleted,
    #[serde(rename = "status.requested")]
    StatusRequested,
    #[serde(rename = "technical")]
    Technical,
    #[serde(rename = "heartbeat")]
    Heartbeat,
}

impl EventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GoalAccepted => "goal.accepted",
            Self::WorkerStarted => "worker.started",
            Self::SourcesMilestone => "sources.milestone",
            Self::FindingMaterial => "finding.material",
            Self::PermissionRequired => "permission.required",
            Self::InputRequired => "input.required",
            Self::BlockerMaterial => "blocker.material",
            Self::GoalCompleted => "goal.completed",
            Self::StatusRequested => "status.requested",
            Self::Technical => "technical",
            Self::Heartbeat => "heartbeat",
        }
    }

    const fn requires_evidence(self) -> bool {
        matches!(
            self,
            Self::WorkerStarted
                | Self::SourcesMilestone
                | Self::FindingMaterial
                | Self::GoalCompleted
        )
    }

    const fn urgent(self) -> bool {
        matches!(
            self,
            Self::PermissionRequired
                | Self::InputRequired
                | Self::BlockerMaterial
                | Self::GoalCompleted
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub goal_id: String,
    pub event_id: String,
    pub kind: EventKind,
    pub occurred_at: f64,
    pub summary: String,
    #[serde(default)]
    pub milestone_key: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub read_source_ids: Vec<String>,
}

impl Event {
    pub fn validate(&self) -> Result<(), CoreError> {
        nonempty(&self.goal_id, "goal_id", MAX_ID_BYTES)?;
        nonempty(&self.event_id, "event_id", MAX_ID_BYTES)?;
        nonempty(&self.summary, "summary", MAX_TEXT_BYTES)?;
        if self.milestone_key.len() > MAX_ID_BYTES {
            return Err(CoreError::SizeLimit {
                field: "milestone_key",
            });
        }
        timestamp(self.occurred_at, "occurred_at")?;
        refs(&self.evidence_refs, "evidence_refs")?;
        refs(&self.read_source_ids, "read_source_ids")?;
        Ok(())
    }

    pub fn notice_key(&self) -> String {
        let key = if self.milestone_key.is_empty() {
            &self.event_id
        } else {
            &self.milestone_key
        };
        format!("{}:{key}", self.kind.as_str())
    }
}

fn default_cooldown() -> f64 {
    45.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub now: f64,
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub user_speaking: bool,
    #[serde(default = "default_cooldown")]
    pub routine_cooldown_seconds: f64,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            now: 0.0,
            quiet: false,
            user_speaking: false,
            routine_cooldown_seconds: default_cooldown(),
        }
    }
}

impl Context {
    pub fn new(now: f64) -> Self {
        Self {
            now,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        timestamp(self.now, "now")?;
        timestamp(self.routine_cooldown_seconds, "routine_cooldown_seconds")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DeliveryState {
    pub surface_delivered: bool,
    pub speech_delivered: bool,
    pub last_speech: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    None,
    Timeline,
    ActionCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speech {
    None,
    Ready,
    Defer,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decision {
    pub surface: Surface,
    pub speech: Speech,
    pub reason: &'static str,
    pub text: String,
    pub notice_key: String,
}

impl Decision {
    fn silent(reason: &'static str) -> Self {
        Self {
            surface: Surface::None,
            speech: Speech::None,
            reason,
            text: String::new(),
            notice_key: String::new(),
        }
    }
}

pub fn decide(
    event: &Event,
    context: &Context,
    delivery: DeliveryState,
) -> Result<Decision, CoreError> {
    event.validate()?;
    context.validate()?;
    if let Some(last) = delivery.last_speech {
        timestamp(last, "last_speech")?;
    }
    if event.occurred_at > context.now {
        return Ok(Decision::silent("future_event"));
    }
    if matches!(event.kind, EventKind::Technical | EventKind::Heartbeat) {
        return Ok(Decision::silent("diagnostic_only"));
    }
    if event.kind.requires_evidence() && !(event.verified && !event.evidence_refs.is_empty()) {
        return Ok(Decision::silent("unverified_claim"));
    }
    let text = if event.kind == EventKind::SourcesMilestone {
        let sources: BTreeSet<&str> = event.read_source_ids.iter().map(String::as_str).collect();
        if sources.len() < 3 {
            return Ok(Decision::silent("below_source_milestone"));
        }
        format!(
            "Đã đối chiếu {} nguồn duy nhất có nội dung đã đọc.",
            sources.len()
        )
    } else {
        event.summary.trim().to_owned()
    };
    let surface = if delivery.surface_delivered {
        Surface::None
    } else if matches!(
        event.kind,
        EventKind::PermissionRequired | EventKind::InputRequired
    ) {
        Surface::ActionCard
    } else {
        Surface::Timeline
    };
    let (speech, reason) = if delivery.speech_delivered {
        (Speech::None, "speech_already_delivered")
    } else if context.quiet && event.kind != EventKind::StatusRequested {
        (Speech::None, "quiet_mode")
    } else if context.user_speaking {
        (Speech::Defer, "user_has_floor")
    } else if !event.kind.urgent()
        && event.kind != EventKind::StatusRequested
        && delivery
            .last_speech
            .is_some_and(|last| context.now - last < context.routine_cooldown_seconds)
    {
        (Speech::Defer, "routine_cooldown")
    } else {
        (Speech::Ready, "meaningful_event")
    };
    Ok(Decision {
        surface,
        speech,
        reason,
        text,
        notice_key: event.notice_key(),
    })
}
