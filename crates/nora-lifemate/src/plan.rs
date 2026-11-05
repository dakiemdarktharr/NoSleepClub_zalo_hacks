//! Experimental, deterministic dataflow executor. Not integrated with the
//! planner or enabled by default. It performs no I/O except the supplied SDK
//! callback and never reads environment files or constructs a state hash.
//!
//! A valid plan is not proof of user intent: literal values, selected tools,
//! conditions and omitted goals still need trustworthy planning and consent.
//! References prove only a data dependency, not the semantic appropriateness
//! of an action. Date arithmetic and foreach are intentionally unsupported.
//!
//! The callback must enforce the supplied remaining duration and bound its
//! response payloads (the existing SDK bridge does both). Synchronous Rust
//! cannot preempt an arbitrary blocking callback. A late callback is recorded,
//! then execution stops before another call; its effects are never erased.
//! Asking for user input is a suspension boundary. Pending input and
//! asynchronous done=false results cannot become successful dataflow sources.

use crate::{
    MAX_JSON_BYTES, MAX_TOOL_CALLS, MAX_TOOLS, SAMPLE_DEADLINE_MS,
    bridge::CallOutcome,
    protocol::{ToolCall, ToolSpec},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    time::{Duration, Instant},
};

pub const MAX_PLAN_STEPS: usize = 64;
pub const MAX_SELECTION_ITEMS: usize = 128;
const MAX_DEPTH: usize = 16;
const MAX_DATA_DEPTH: usize = 32;
const MAX_NODES: usize = 16_384;
const MAX_FIELDS: usize = 64;
const MAX_FILTERS: usize = 16;
const MAX_POINTER_BYTES: usize = 512;
const MAX_TOTAL_OUTPUT_BYTES: usize = 8 * MAX_JSON_BYTES;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Call {
        id: String,
        tool: String,
        arguments: BTreeMap<String, Expr>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Predicate>,
    },
    SelectUnique {
        id: String,
        source: Expr,
        filters: Vec<FieldEquals>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Predicate>,
    },
}

impl Step {
    fn id(&self) -> &str {
        match self {
            Self::Call { id, .. } | Self::SelectUnique { id, .. } => id,
        }
    }

    fn when(&self) -> Option<&Predicate> {
        match self {
            Self::Call { when, .. } | Self::SelectUnique { when, .. } => when.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    Literal { value: Value },
    Ref { step: String, pointer: String },
    Array { items: Vec<Expr> },
    Object { fields: BTreeMap<String, Expr> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
    Boolean { value: Expr },
    Equals { left: Expr, right: Expr },
    All { predicates: Vec<Predicate> },
    Any { predicates: Vec<Predicate> },
    Not { predicate: Box<Predicate> },
}

/// All filters must match. Missing fields do not match; values use strict JSON
/// equality without case folding, coercion or changes to the returned record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldEquals {
    pub pointer: String,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    InvalidPlan,
    Bounds,
    UnknownTool,
    DuplicateStep,
    MissingReference,
    ForwardReference,
    InvalidPointer,
    ReferenceUnavailable,
    PointerMissing,
    TypeMismatch,
    NoSelection,
    AmbiguousSelection,
    CallRejected,
    CallFailed,
    CallIncomplete,
    CallbackFailed,
    Deadline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind:?} at {step:?}: {detail}")]
pub struct PlanError {
    pub kind: ErrorKind,
    pub step: Option<String>,
    pub detail: String,
}

fn error(kind: ErrorKind, step: Option<&str>, detail: &str) -> PlanError {
    PlanError {
        kind,
        step: step.map(str::to_owned),
        detail: detail.to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Succeeded,
    Skipped,
    NeedsInput,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepRecord {
    pub id: String,
    pub status: StepStatus,
    pub result: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallRecord {
    pub step: String,
    pub call: ToolCall,
    /// None means callback transport failed; execution cannot be established.
    pub executed: Option<bool>,
    pub validation: Option<Value>,
    pub result: Option<Value>,
    pub transport_error: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExecutionReport {
    /// Reached the plan's end without pending input; not proof of user intent.
    pub completed: bool,
    /// An executed ask/input-wait boundary was observed. May coexist with an
    /// error; this flag alone does not establish delivery of a valid question.
    #[serde(default)]
    pub needs_input: bool,
    pub steps: Vec<StepRecord>,
    pub calls: Vec<CallRecord>,
    pub error: Option<PlanError>,
}

struct Validation<'a> {
    all: &'a BTreeSet<String>,
    earlier: &'a BTreeSet<String>,
    step: &'a str,
    nodes: &'a mut usize,
    literal_bytes: &'a mut usize,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn pointer_valid(pointer: &str) -> bool {
    if pointer.len() > MAX_POINTER_BYTES
        || !pointer.is_empty() && !pointer.starts_with('/')
        || pointer.chars().any(char::is_control)
        || pointer.bytes().filter(|byte| *byte == b'/').count() > MAX_DATA_DEPTH
    {
        return false;
    }
    let mut bytes = pointer.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return false;
        }
    }
    true
}

fn check_value(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), ()> {
    *nodes += 1;
    if depth > MAX_DATA_DEPTH || *nodes > MAX_NODES {
        return Err(());
    }
    match value {
        Value::Array(items) => {
            for item in items {
                check_value(item, depth + 1, nodes)?;
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                if key.len() > MAX_JSON_BYTES {
                    return Err(());
                }
                check_value(value, depth + 1, nodes)?;
            }
        }
        Value::String(value) if value.len() > MAX_JSON_BYTES => return Err(()),
        _ => {}
    }
    Ok(())
}

fn bounded_size(value: &Value) -> Result<usize, ()> {
    struct Counter(usize);
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_JSON_BYTES.saturating_sub(self.0) {
                return Err(std::io::Error::other("plan value exceeds size bound"));
            }
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    check_value(value, 0, &mut 0)?;
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_err(|_| ())?;
    Ok(counter.0)
}

impl Validation<'_> {
    fn node(&mut self, depth: usize) -> Result<(), PlanError> {
        *self.nodes += 1;
        if depth > MAX_DEPTH || *self.nodes > MAX_NODES {
            return Err(error(
                ErrorKind::Bounds,
                Some(self.step),
                "expression bounds",
            ));
        }
        Ok(())
    }

    fn expr(&mut self, expr: &Expr, depth: usize) -> Result<(), PlanError> {
        self.node(depth)?;
        match expr {
            Expr::Literal { value } => {
                let size = bounded_size(value)
                    .map_err(|()| error(ErrorKind::Bounds, Some(self.step), "literal bounds"))?;
                *self.literal_bytes += size;
                if *self.literal_bytes > MAX_JSON_BYTES {
                    return Err(error(
                        ErrorKind::Bounds,
                        Some(self.step),
                        "total literal size",
                    ));
                }
            }
            Expr::Ref { step, pointer } => {
                if !pointer_valid(pointer) {
                    return Err(error(
                        ErrorKind::InvalidPointer,
                        Some(self.step),
                        "invalid JSON pointer",
                    ));
                }
                if !self.earlier.contains(step) {
                    let kind = if self.all.contains(step) {
                        ErrorKind::ForwardReference
                    } else {
                        ErrorKind::MissingReference
                    };
                    return Err(error(
                        kind,
                        Some(self.step),
                        "reference must name an earlier step",
                    ));
                }
            }
            Expr::Array { items } => {
                if items.len() > MAX_SELECTION_ITEMS {
                    return Err(error(
                        ErrorKind::Bounds,
                        Some(self.step),
                        "array expression size",
                    ));
                }
                for item in items {
                    self.expr(item, depth + 1)?;
                }
            }
            Expr::Object { fields } => self.fields(fields, depth + 1)?,
        }
        Ok(())
    }

    fn fields(&mut self, fields: &BTreeMap<String, Expr>, depth: usize) -> Result<(), PlanError> {
        if fields.len() > MAX_FIELDS || fields.keys().any(|key| key.len() > 128) {
            return Err(error(
                ErrorKind::Bounds,
                Some(self.step),
                "object expression size",
            ));
        }
        for expr in fields.values() {
            self.expr(expr, depth)?;
        }
        Ok(())
    }

    fn predicate(&mut self, predicate: &Predicate, depth: usize) -> Result<(), PlanError> {
        self.node(depth)?;
        match predicate {
            Predicate::Boolean { value } => self.expr(value, depth + 1)?,
            Predicate::Equals { left, right } => {
                self.expr(left, depth + 1)?;
                self.expr(right, depth + 1)?;
            }
            Predicate::All { predicates } | Predicate::Any { predicates } => {
                if predicates.is_empty() || predicates.len() > MAX_FILTERS {
                    return Err(error(ErrorKind::Bounds, Some(self.step), "predicate count"));
                }
                for predicate in predicates {
                    self.predicate(predicate, depth + 1)?;
                }
            }
            Predicate::Not { predicate } => self.predicate(predicate, depth + 1)?,
        }
        Ok(())
    }
}

impl Plan {
    /// Validate the whole plan before any callback, including skipped branches.
    pub fn validate(&self, tools: &[ToolSpec]) -> Result<(), PlanError> {
        if self.steps.len() > MAX_PLAN_STEPS || tools.len() > MAX_TOOLS {
            return Err(error(ErrorKind::Bounds, None, "step/catalog count"));
        }
        let mut tool_names = BTreeSet::new();
        if tools
            .iter()
            .any(|tool| !tool_names.insert(tool.name.as_str()))
        {
            return Err(error(
                ErrorKind::InvalidPlan,
                None,
                "duplicate catalog tool",
            ));
        }
        let mut all = BTreeSet::new();
        for step in &self.steps {
            if !valid_id(step.id()) {
                return Err(error(ErrorKind::InvalidPlan, None, "invalid step ID"));
            }
            if !all.insert(step.id().to_owned()) {
                return Err(error(
                    ErrorKind::DuplicateStep,
                    Some(step.id()),
                    "duplicate step ID",
                ));
            }
        }
        let mut earlier = BTreeSet::new();
        let mut calls = 0;
        let mut nodes = 0;
        let mut literal_bytes = 0;
        for step in &self.steps {
            let mut validation = Validation {
                all: &all,
                earlier: &earlier,
                step: step.id(),
                nodes: &mut nodes,
                literal_bytes: &mut literal_bytes,
            };
            if let Some(when) = step.when() {
                validation.predicate(when, 0)?;
            }
            match step {
                Step::Call {
                    tool, arguments, ..
                } => {
                    calls += 1;
                    if calls > MAX_TOOL_CALLS {
                        return Err(error(ErrorKind::Bounds, Some(step.id()), "call count"));
                    }
                    if !tool_names.contains(tool.as_str()) {
                        return Err(error(
                            ErrorKind::UnknownTool,
                            Some(step.id()),
                            "tool is not allowed",
                        ));
                    }
                    validation.fields(arguments, 0)?;
                }
                Step::SelectUnique {
                    source, filters, ..
                } => {
                    validation.expr(source, 0)?;
                    if filters.len() > MAX_FILTERS {
                        return Err(error(
                            ErrorKind::Bounds,
                            Some(step.id()),
                            "selection filter count",
                        ));
                    }
                    for filter in filters {
                        if !pointer_valid(&filter.pointer) {
                            return Err(error(
                                ErrorKind::InvalidPointer,
                                Some(step.id()),
                                "invalid filter pointer",
                            ));
                        }
                        validation.expr(&filter.value, 0)?;
                    }
                }
            }
            earlier.insert(step.id().to_owned());
        }
        Ok(())
    }
}

fn eval(expr: &Expr, steps: &[StepRecord], id: &str) -> Result<Value, PlanError> {
    let mut budget = MAX_JSON_BYTES;
    eval_bounded(expr, steps, id, &mut budget)
}

fn spend(budget: &mut usize, size: usize, id: &str) -> Result<(), PlanError> {
    *budget = budget
        .checked_sub(size)
        .ok_or_else(|| error(ErrorKind::Bounds, Some(id), "resolved expression size"))?;
    Ok(())
}

fn clone_bounded(value: &Value, budget: &mut usize, id: &str) -> Result<Value, PlanError> {
    let size = bounded_size(value)
        .map_err(|()| error(ErrorKind::Bounds, Some(id), "referenced/literal value size"))?;
    spend(budget, size, id)?;
    Ok(value.clone())
}

fn eval_fields(
    fields: &BTreeMap<String, Expr>,
    steps: &[StepRecord],
    id: &str,
    budget: &mut usize,
) -> Result<serde_json::Map<String, Value>, PlanError> {
    spend(budget, 2 + fields.len().saturating_sub(1), id)?;
    let mut result = serde_json::Map::new();
    for (key, expr) in fields {
        let key_size = bounded_size(&Value::String(key.clone()))
            .map_err(|()| error(ErrorKind::Bounds, Some(id), "resolved object key size"))?;
        spend(budget, key_size + 1, id)?;
        result.insert(key.clone(), eval_bounded(expr, steps, id, budget)?);
    }
    Ok(result)
}

fn eval_bounded(
    expr: &Expr,
    steps: &[StepRecord],
    id: &str,
    budget: &mut usize,
) -> Result<Value, PlanError> {
    match expr {
        Expr::Literal { value } => clone_bounded(value, budget, id),
        Expr::Ref { step, pointer } => {
            let record = steps
                .iter()
                .find(|record| record.id == *step)
                .filter(|record| record.status == StepStatus::Succeeded)
                .and_then(|record| record.result.as_ref())
                .ok_or_else(|| {
                    error(
                        ErrorKind::ReferenceUnavailable,
                        Some(id),
                        "referenced step has no successful result",
                    )
                })?;
            let value = record.pointer(pointer).ok_or_else(|| {
                error(
                    ErrorKind::PointerMissing,
                    Some(id),
                    "pointer is absent from the referenced result",
                )
            })?;
            clone_bounded(value, budget, id)
        }
        Expr::Array { items } => {
            spend(budget, 2 + items.len().saturating_sub(1), id)?;
            items
                .iter()
                .map(|item| eval_bounded(item, steps, id, budget))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        Expr::Object { fields } => eval_fields(fields, steps, id, budget).map(Value::Object),
    }
}

fn eval_predicate(
    condition: &Predicate,
    steps: &[StepRecord],
    id: &str,
) -> Result<bool, PlanError> {
    match condition {
        Predicate::Boolean { value } => eval(value, steps, id)?.as_bool().ok_or_else(|| {
            error(
                ErrorKind::TypeMismatch,
                Some(id),
                "boolean gate requires an actual JSON boolean",
            )
        }),
        Predicate::Equals { left, right } => Ok(eval(left, steps, id)? == eval(right, steps, id)?),
        Predicate::All { predicates } => {
            for condition in predicates {
                if !eval_predicate(condition, steps, id)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Predicate::Any { predicates } => {
            for condition in predicates {
                if eval_predicate(condition, steps, id)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Predicate::Not {
            predicate: condition,
        } => Ok(!eval_predicate(condition, steps, id)?),
    }
}

fn remaining(deadline: Instant, step: &str) -> Result<Duration, PlanError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(error(
            ErrorKind::Deadline,
            Some(step),
            "plan deadline exceeded",
        ));
    }
    Ok(remaining)
}

fn select_unique(
    source: Value,
    filters: &[FieldEquals],
    steps: &[StepRecord],
    id: &str,
) -> Result<Value, PlanError> {
    let items = source.as_array().ok_or_else(|| {
        error(
            ErrorKind::TypeMismatch,
            Some(id),
            "selection source must be an array",
        )
    })?;
    if items.len() > MAX_SELECTION_ITEMS {
        return Err(error(
            ErrorKind::Bounds,
            Some(id),
            "selection candidate count",
        ));
    }
    let filters = filters
        .iter()
        .map(|filter| Ok((filter.pointer.as_str(), eval(&filter.value, steps, id)?)))
        .collect::<Result<Vec<_>, PlanError>>()?;
    let matches: Vec<_> = items
        .iter()
        .filter(|item| {
            filters
                .iter()
                .all(|(pointer, expected)| item.pointer(pointer) == Some(expected))
        })
        .collect();
    match matches.as_slice() {
        [] => Err(error(
            ErrorKind::NoSelection,
            Some(id),
            "no record matches all filters",
        )),
        [selected] => Ok((*selected).clone()),
        _ => Err(error(
            ErrorKind::AmbiguousSelection,
            Some(id),
            "multiple records match; disambiguation is required",
        )),
    }
}

/// Execute one prevalidated plan. Failed or uncertain calls retain their
/// original arguments, validation and results in the partial report.
pub fn execute<F>(
    plan: &Plan,
    tools: &[ToolSpec],
    deadline: Instant,
    mut call: F,
) -> ExecutionReport
where
    F: FnMut(&ToolCall, Duration) -> crate::Result<CallOutcome>,
{
    let mut report = ExecutionReport::default();
    let deadline = deadline.min(Instant::now() + Duration::from_millis(SAMPLE_DEADLINE_MS));
    if Instant::now() >= deadline {
        report.error = Some(error(ErrorKind::Deadline, None, "plan deadline exceeded"));
        return report;
    }
    if let Err(problem) = plan.validate(tools) {
        report.error = Some(problem);
        return report;
    }
    let mut total_output_bytes = 0usize;
    for step in &plan.steps {
        let id = step.id();
        let result = (|| -> Result<Option<Value>, PlanError> {
            remaining(deadline, id)?;
            if let Some(when) = step.when() {
                let enabled = eval_predicate(when, &report.steps, id)?;
                remaining(deadline, id)?;
                if !enabled {
                    return Ok(None);
                }
            }
            let result = match step {
                Step::SelectUnique {
                    source, filters, ..
                } => select_unique(eval(source, &report.steps, id)?, filters, &report.steps, id)?,
                Step::Call {
                    tool, arguments, ..
                } => {
                    if report.calls.len() >= MAX_TOOL_CALLS {
                        return Err(error(ErrorKind::Bounds, Some(id), "runtime call count"));
                    }
                    let mut argument_budget = MAX_JSON_BYTES;
                    let arguments =
                        eval_fields(arguments, &report.steps, id, &mut argument_budget)?;
                    let arguments = Value::Object(arguments);
                    bounded_size(&arguments).map_err(|()| {
                        error(ErrorKind::Bounds, Some(id), "resolved argument bounds")
                    })?;
                    let prepared = ToolCall {
                        name: tool.clone(),
                        arguments,
                    };
                    let time_left = remaining(deadline, id)?;
                    report.calls.push(CallRecord {
                        step: id.to_owned(),
                        call: prepared,
                        executed: None,
                        validation: None,
                        result: None,
                        transport_error: None,
                    });
                    let recorded = report
                        .calls
                        .last_mut()
                        .expect("just inserted callback record");
                    let outcome = match call(&recorded.call, time_left) {
                        Ok(outcome) => outcome,
                        Err(problem) => {
                            recorded.transport_error = Some(problem.to_string());
                            return Err(error(
                                ErrorKind::CallbackFailed,
                                Some(id),
                                "callback failed; effect execution is uncertain",
                            ));
                        }
                    };
                    recorded.executed = Some(outcome.executed);
                    recorded.validation = Some(outcome.validation);
                    recorded.result = outcome.result;
                    report.needs_input = outcome.executed
                        && (recorded.call.name == "sys.ask_missing_info"
                            || recorded
                                .result
                                .as_ref()
                                .and_then(|value| value.get("waiting_for_user"))
                                .and_then(Value::as_bool)
                                == Some(true));
                    remaining(deadline, id)?;
                    if !outcome.executed {
                        return Err(error(
                            ErrorKind::CallRejected,
                            Some(id),
                            "SDK did not execute the call",
                        ));
                    }
                    if recorded
                        .validation
                        .as_ref()
                        .and_then(|value| value.get("ok"))
                        .and_then(Value::as_bool)
                        != Some(true)
                    {
                        return Err(error(
                            ErrorKind::CallFailed,
                            Some(id),
                            "executed call lacks successful SDK validation",
                        ));
                    }
                    let result = recorded
                        .result
                        .as_ref()
                        .filter(|result| result.get("ok").and_then(Value::as_bool) == Some(true))
                        .ok_or_else(|| {
                            error(
                                ErrorKind::CallFailed,
                                Some(id),
                                "executed call did not return ok=true",
                            )
                        })?;
                    if !report.needs_input
                        && result.get("done").and_then(Value::as_bool) == Some(false)
                    {
                        return Err(error(
                            ErrorKind::CallIncomplete,
                            Some(id),
                            "asynchronous/incomplete call requires a new plan",
                        ));
                    }
                    bounded_size(result)
                        .map_err(|()| error(ErrorKind::Bounds, Some(id), "SDK result bounds"))?;
                    result.clone()
                }
            };
            let size = bounded_size(&result)
                .map_err(|()| error(ErrorKind::Bounds, Some(id), "step result bounds"))?;
            total_output_bytes += size;
            if total_output_bytes > MAX_TOTAL_OUTPUT_BYTES {
                return Err(error(
                    ErrorKind::Bounds,
                    Some(id),
                    "total retained output size",
                ));
            }
            remaining(deadline, id)?;
            Ok(Some(result))
        })();
        match result {
            Ok(result) => {
                report.steps.push(StepRecord {
                    id: id.to_owned(),
                    status: if report.needs_input {
                        StepStatus::NeedsInput
                    } else if result.is_some() {
                        StepStatus::Succeeded
                    } else {
                        StepStatus::Skipped
                    },
                    result,
                });
                if report.needs_input {
                    return report;
                }
            }
            Err(problem) => {
                report.steps.push(StepRecord {
                    id: id.to_owned(),
                    status: StepStatus::Failed,
                    result: None,
                });
                report.error = Some(problem);
                return report;
            }
        }
    }
    report.completed = true;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plan(value: Value) -> Plan {
        serde_json::from_value(value).unwrap()
    }
    fn tools() -> Vec<ToolSpec> {
        [
            "weather.get_forecast",
            "contacts.search",
            "phone.call",
            "smart_home.light_set",
            "sys.ask_missing_info",
        ]
        .iter()
        .map(|name| ToolSpec {
            name: (*name).into(),
            kind: "toy".into(),
            description: "Original controller fixture".into(),
            parameters: json!({"type":"object"}),
            returns: Value::Null,
        })
        .collect()
    }
    fn ok(result: Value) -> crate::Result<CallOutcome> {
        Ok(CallOutcome {
            executed: true,
            validation: json!({"ok":true}),
            result: Some(result),
        })
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(1)
    }

    #[test]
    fn false_weather_gate_skips_the_effect_without_a_callback() {
        let plan = plan(json!({"steps":[
            {"op":"call","id":"weather","tool":"weather.get_forecast","arguments":{}},
            {"op":"call","id":"effect","tool":"smart_home.light_set","arguments":{
                "device_id":{"kind":"literal","value":"light_1"},"power":{"kind":"literal","value":"off"}},
                "when":{"kind":"boolean","value":{"kind":"ref","step":"weather","pointer":"/forecast/0/rain"}}}
        ]}));
        let report = execute(&plan, &tools(), deadline(), |call, _| {
            assert_eq!(call.name, "weather.get_forecast");
            ok(json!({"ok":true,"forecast":[{"rain":false}]}))
        });
        assert!(report.completed);
        assert_eq!(report.calls.len(), 1);
        assert_eq!(report.steps[1].status, StepStatus::Skipped);
        assert!(report.error.is_none());
    }

    #[test]
    fn equality_and_boolean_composition_never_coerce_text_to_truth() {
        let composed = plan(
            json!({"steps":[{"op":"call","id":"read","tool":"contacts.search","arguments":{},
            "when":{"kind":"all","predicates":[
                {"kind":"equals","left":{"kind":"literal","value":4},"right":{"kind":"literal","value":4}},
                {"kind":"not","predicate":{"kind":"boolean","value":{"kind":"literal","value":false}}}
            ]}}]}),
        );
        let report = execute(&composed, &tools(), deadline(), |_, _| {
            ok(json!({"ok":true}))
        });
        assert!(report.completed);
        assert_eq!(report.calls.len(), 1);
        let wrong_type = plan(
            json!({"steps":[{"op":"call","id":"read","tool":"contacts.search","arguments":{},
            "when":{"kind":"boolean","value":{"kind":"literal","value":"true"}}}]}),
        );
        let report = execute(&wrong_type, &tools(), deadline(), |_, _| {
            panic!("a string is not a boolean")
        });
        assert_eq!(report.error.unwrap().kind, ErrorKind::TypeMismatch);
        assert!(report.calls.is_empty());
    }

    #[test]
    fn repeated_references_are_bounded_before_constructing_an_effect() {
        let result = json!({"ok":true,"payload":"v".repeat(50_000)});
        let mut arguments = serde_json::Map::new();
        for index in 0..MAX_FIELDS {
            arguments.insert(
                format!("field_{index}"),
                json!({"kind":"ref","step":"read","pointer":"/payload"}),
            );
        }
        let plan = plan(json!({"steps":[
            {"op":"call","id":"read","tool":"contacts.search","arguments":{}},
            {"op":"call","id":"effect","tool":"phone.call","arguments":arguments}
        ]}));
        let report = execute(&plan, &tools(), deadline(), |call, _| {
            assert_eq!(call.name, "contacts.search");
            ok(result.clone())
        });
        assert_eq!(report.error.unwrap().kind, ErrorKind::Bounds);
        assert_eq!(report.calls.len(), 1);
        assert_eq!(report.calls[0].result, Some(result));
    }

    #[test]
    fn unique_selection_preserves_the_record_and_binds_later_arguments() {
        let selected =
            json!({"id":"person_2","name":"Minh","relation":"colleague","extra":{"keep":true}});
        let contacts = json!({"ok":true,"contacts":[{"id":"person_1","name":"Minh","relation":"customer"},selected]});
        let plan = plan(json!({"steps":[
            {"op":"call","id":"lookup","tool":"contacts.search","arguments":{}},
            {"op":"select_unique","id":"person","source":{"kind":"ref","step":"lookup","pointer":"/contacts"},
                "filters":[{"pointer":"/name","value":{"kind":"literal","value":"Minh"}},
                    {"pointer":"/relation","value":{"kind":"literal","value":"colleague"}}]},
            {"op":"call","id":"dial","tool":"phone.call","arguments":{"contact_id":{"kind":"ref","step":"person","pointer":"/id"}}}
        ]}));
        let report = execute(&plan, &tools(), deadline(), |call, _| {
            if call.name == "contacts.search" {
                ok(contacts.clone())
            } else {
                assert_eq!(call.arguments, json!({"contact_id":"person_2"}));
                ok(json!({"ok":true}))
            }
        });
        assert!(report.completed);
        assert_eq!(report.calls.len(), 2);
        assert_eq!(report.steps[1].result, Some(selected));
        assert_eq!(report.calls[0].result, Some(contacts));
    }

    #[test]
    fn ambiguous_contacts_stop_before_dialing() {
        let plan = plan(json!({"steps":[
            {"op":"call","id":"lookup","tool":"contacts.search","arguments":{}},
            {"op":"select_unique","id":"person","source":{"kind":"ref","step":"lookup","pointer":"/contacts"},
                "filters":[{"pointer":"/name","value":{"kind":"literal","value":"Minh"}}]},
            {"op":"call","id":"dial","tool":"phone.call","arguments":{"contact_id":{"kind":"ref","step":"person","pointer":"/id"}}}
        ]}));
        let report = execute(&plan, &tools(), deadline(), |call, _| {
            assert_eq!(call.name, "contacts.search");
            ok(
                json!({"ok":true,"contacts":[{"id":"person_1","name":"Minh"},{"id":"person_2","name":"Minh"}]}),
            )
        });
        assert_eq!(report.error.unwrap().kind, ErrorKind::AmbiguousSelection);
        assert_eq!(report.calls.len(), 1);
        assert!(!report.completed);
    }

    #[test]
    fn asking_or_waiting_for_user_input_suspends_before_a_later_effect() {
        for (tool, result) in [
            ("sys.ask_missing_info", json!({"ok":true})),
            (
                "contacts.search",
                json!({"ok":true,"waiting_for_user":true}),
            ),
        ] {
            let arguments = if tool == "sys.ask_missing_info" {
                json!({"tool_name":{"kind":"literal","value":"phone.call"},
                    "params":{"kind":"literal","value":["contact_id"]}})
            } else {
                json!({})
            };
            let plan = plan(json!({"steps":[
                {"op":"call","id":"question","tool":tool,"arguments":arguments},
                {"op":"call","id":"later","tool":"phone.call","arguments":{
                    "contact_id":{"kind":"literal","value":"person_1"}}}
            ]}));
            let report = execute(&plan, &tools(), deadline(), |call, _| {
                assert_eq!(call.name, tool);
                ok(result.clone())
            });
            assert!(report.needs_input);
            assert!(!report.completed);
            assert!(report.error.is_none());
            assert_eq!(report.calls.len(), 1);
            assert_eq!(report.calls[0].executed, Some(true));
            assert_eq!(report.calls[0].result, Some(result.clone()));
            assert_eq!(report.steps.len(), 1);
            assert_eq!(report.steps[0].status, StepStatus::NeedsInput);
            assert_eq!(report.steps[0].result, Some(result));
        }
    }

    #[test]
    fn done_false_keeps_actual_evidence_without_unlocking_a_later_reference() {
        let plan = plan(json!({"steps":[
            {"op":"call","id":"pending","tool":"contacts.search","arguments":{}},
            {"op":"call","id":"later","tool":"phone.call","arguments":{
                "contact_id":{"kind":"ref","step":"pending","pointer":"/id"}}}
        ]}));
        let result = json!({"ok":true,"done":false,"id":"not_available_yet"});
        let report = execute(&plan, &tools(), deadline(), |call, _| {
            assert_eq!(call.name, "contacts.search");
            ok(result.clone())
        });
        assert_eq!(report.error.unwrap().kind, ErrorKind::CallIncomplete);
        assert!(!report.completed);
        assert!(!report.needs_input);
        assert_eq!(report.calls.len(), 1);
        assert_eq!(report.calls[0].executed, Some(true));
        assert_eq!(report.calls[0].result, Some(result));
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].status, StepStatus::Failed);
        assert!(report.steps[0].result.is_none());
    }

    #[test]
    fn invalid_plan_graphs_and_unknown_operations_fail_before_effects() {
        let cases = [
            (
                json!({"steps":[{"op":"call","id":"a","tool":"shell.exec","arguments":{}}]}),
                ErrorKind::UnknownTool,
            ),
            (
                json!({"steps":[{"op":"call","id":"a","tool":"phone.call","arguments":{"x":{"kind":"ref","step":"missing","pointer":""}}}]}),
                ErrorKind::MissingReference,
            ),
            (
                json!({"steps":[{"op":"call","id":"a","tool":"phone.call","arguments":{"x":{"kind":"ref","step":"b","pointer":""}}},
                {"op":"call","id":"b","tool":"contacts.search","arguments":{}}]}),
                ErrorKind::ForwardReference,
            ),
            (
                json!({"steps":[{"op":"call","id":"a","tool":"phone.call","arguments":{}},
                {"op":"call","id":"a","tool":"contacts.search","arguments":{}}]}),
                ErrorKind::DuplicateStep,
            ),
            (
                json!({"steps":[{"op":"call","id":"a","tool":"contacts.search","arguments":{}},
                {"op":"call","id":"b","tool":"phone.call","arguments":{"x":{"kind":"ref","step":"a","pointer":"/bad~2escape"}}}]}),
                ErrorKind::InvalidPointer,
            ),
        ];
        for (value, kind) in cases {
            let report = execute(&plan(value), &tools(), deadline(), |_, _| {
                panic!("invalid plan called the SDK")
            });
            assert_eq!(report.error.unwrap().kind, kind);
            assert!(report.calls.is_empty());
        }
        assert!(
            serde_json::from_value::<Plan>(
                json!({"steps":[{"op":"eval","id":"a","code":"anything"}]})
            )
            .is_err()
        );
    }

    #[test]
    fn skipped_results_and_missing_pointers_cannot_feed_effects() {
        for (when, pointer, expected) in [
            (
                Some(json!({"kind":"boolean","value":{"kind":"literal","value":false}})),
                "",
                ErrorKind::ReferenceUnavailable,
            ),
            (None, "/missing", ErrorKind::PointerMissing),
        ] {
            let plan = plan(json!({"steps":[
                {"op":"call","id":"source","tool":"contacts.search","arguments":{},"when":when},
                {"op":"call","id":"effect","tool":"phone.call","arguments":{"contact_id":{"kind":"ref","step":"source","pointer":pointer}}}
            ]}));
            let report = execute(&plan, &tools(), deadline(), |call, _| {
                assert_eq!(call.name, "contacts.search");
                ok(json!({"ok":true}))
            });
            assert_eq!(report.error.unwrap().kind, expected);
            assert!(
                report
                    .calls
                    .iter()
                    .all(|record| record.call.name == "contacts.search")
            );
        }
    }

    #[test]
    fn call_limit_and_expired_deadline_prevent_callbacks() {
        let steps = (0..=MAX_TOOL_CALLS)
            .map(|index| {
                json!({"op":"call","id":format!("step_{index}"),
            "tool":"contacts.search","arguments":{}})
            })
            .collect::<Vec<_>>();
        let report = execute(
            &plan(json!({"steps":steps})),
            &tools(),
            deadline(),
            |_, _| panic!("call limit bypass"),
        );
        assert_eq!(report.error.unwrap().kind, ErrorKind::Bounds);
        let plan = plan(
            json!({"steps":[{"op":"call","id":"read","tool":"contacts.search","arguments":{}}]}),
        );
        let report = execute(&plan, &tools(), Instant::now(), |_, _| {
            panic!("deadline bypass")
        });
        assert_eq!(report.error.unwrap().kind, ErrorKind::Deadline);
        assert!(report.calls.is_empty());
    }

    #[test]
    fn late_callbacks_keep_their_actual_results_and_stop_later_effects() {
        let plan = plan(json!({"steps":[
            {"op":"call","id":"first","tool":"contacts.search","arguments":{}},
            {"op":"call","id":"later","tool":"phone.call","arguments":{}}
        ]}));
        let result = json!({"ok":true,"id":"returned_unchanged"});
        let report = execute(
            &plan,
            &tools(),
            Instant::now() + Duration::from_millis(100),
            |call, remaining| {
                assert_eq!(call.name, "contacts.search");
                assert!(remaining <= Duration::from_millis(100));
                std::thread::sleep(Duration::from_millis(110));
                ok(result.clone())
            },
        );
        assert_eq!(report.error.unwrap().kind, ErrorKind::Deadline);
        assert_eq!(report.calls.len(), 1);
        assert_eq!(report.calls[0].executed, Some(true));
        assert_eq!(report.calls[0].result, Some(result));
    }

    #[test]
    fn rejected_failed_and_uncertain_callbacks_never_unlock_references() {
        let plan = plan(json!({"steps":[
            {"op":"call","id":"first","tool":"contacts.search","arguments":{}},
            {"op":"call","id":"later","tool":"phone.call","arguments":{"x":{"kind":"ref","step":"first","pointer":"/id"}}}
        ]}));
        for kind in [
            ErrorKind::CallRejected,
            ErrorKind::CallFailed,
            ErrorKind::CallIncomplete,
            ErrorKind::CallbackFailed,
        ] {
            let report = execute(&plan, &tools(), deadline(), |_, _| match kind {
                ErrorKind::CallRejected => Ok(CallOutcome {
                    executed: false,
                    validation: json!({"ok":false}),
                    result: None,
                }),
                ErrorKind::CallFailed => ok(json!({"ok":false,"id":"must_not_flow"})),
                ErrorKind::CallIncomplete => {
                    ok(json!({"ok":true,"done":false,"id":"must_not_flow"}))
                }
                _ => Err("original callback transport failure".into()),
            });
            assert_eq!(report.error.unwrap().kind, kind);
            assert_eq!(report.calls.len(), 1);
            assert!(!report.completed);
            if kind == ErrorKind::CallbackFailed {
                assert_eq!(report.calls[0].executed, None);
            }
        }
    }
}
