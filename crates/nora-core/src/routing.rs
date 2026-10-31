//! Execution complexity and risk gates are independent. Inputs are structured
//! facts from the trusted application, not an NLP classifier or authorization.

use serde::{Deserialize, Serialize};

use crate::{CoreError, MAX_ID_BYTES, nonempty};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "answer")]
    Answer,
    #[serde(rename = "bounded_tool")]
    Tool,
    #[serde(rename = "durable_goal")]
    Goal,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gate {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "waiting_input")]
    Input,
    #[serde(rename = "waiting_permission")]
    Permission,
    #[serde(rename = "waiting_approval")]
    Approval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TaskFacts {
    pub requires_tools: bool,
    pub tool_supported: bool,
    pub expected_steps: u32,
    pub has_dependencies: bool,
    pub requires_resume: bool,
    pub open_ended_discovery: bool,
    pub missing_parameters: Vec<String>,
    pub critical_slot_ambiguous: bool,
    pub needs_new_grant: bool,
    pub needs_effect_approval: bool,
}

impl Default for TaskFacts {
    fn default() -> Self {
        Self {
            requires_tools: false,
            tool_supported: true,
            expected_steps: 0,
            has_dependencies: false,
            requires_resume: false,
            open_ended_discovery: false,
            missing_parameters: Vec::new(),
            critical_slot_ambiguous: false,
            needs_new_grant: false,
            needs_effect_approval: false,
        }
    }
}

impl TaskFacts {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.missing_parameters.len() > 64 {
            return Err(CoreError::SizeLimit {
                field: "missing_parameters",
            });
        }
        for name in &self.missing_parameters {
            nonempty(name, "missing_parameters", MAX_ID_BYTES)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Route {
    pub mode: Mode,
    pub gate: Gate,
    pub reason: &'static str,
}

pub fn route(facts: &TaskFacts) -> Result<Route, CoreError> {
    facts.validate()?;
    if facts.requires_tools && !facts.tool_supported {
        return Ok(Route {
            mode: Mode::Unsupported,
            gate: Gate::Ready,
            reason: "no_supported_tool",
        });
    }
    let complex = facts.requires_resume
        || facts.has_dependencies
        || facts.open_ended_discovery
        || facts.expected_steps >= 4;
    let mode = if complex {
        Mode::Goal
    } else if facts.requires_tools {
        Mode::Tool
    } else {
        Mode::Answer
    };
    let (gate, reason) = if !facts.missing_parameters.is_empty() || facts.critical_slot_ambiguous {
        (Gate::Input, "missing_or_ambiguous_input")
    } else if facts.needs_new_grant {
        (Gate::Permission, "new_capability_required")
    } else if facts.needs_effect_approval {
        (Gate::Approval, "exact_effect_preview_required")
    } else {
        (
            Gate::Ready,
            if complex {
                "durability_required"
            } else {
                "bounded"
            },
        )
    };
    Ok(Route { mode, gate, reason })
}
