//! Typed broker checks over trusted grants and provenance.
//!
//! These functions do not authenticate users, resolve filesystem paths, enforce
//! network/OS isolation, classify secrets or detect prompt injection semantically.
//! Resolve canonical targets and obtain grants/labels/evidence from trusted
//! broker/ingestion code, never from a worker's self-declared JSON fields.

use std::collections::BTreeMap;
use std::io::{self, Write};

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{CoreError, MAX_ID_BYTES, MAX_JSON_BYTES, MAX_REFS, MAX_TEXT_BYTES, nonempty};

const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_NODES: usize = 4096;
const MAX_FIELDS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Personal,
    Confidential,
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    LocalRead,
    LocalWrite,
    Network,
    ModelRequest,
}

impl EffectKind {
    pub fn is_egress(self) -> bool {
        matches!(self, Self::Network | Self::ModelRequest)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionProposal {
    pub action_id: String,
    pub goal_id: String,
    pub goal_version: u64,
    pub account: String,
    pub tool: String,
    pub effect: EffectKind,
    /// Adapter-resolved identity, not an unchecked path/display name/raw URL.
    pub canonical_target: String,
    pub arguments: Value,
}

impl ActionProposal {
    pub fn validate(&self) -> Result<(), CoreError> {
        for (field, text) in [
            ("action_id", &self.action_id),
            ("goal_id", &self.goal_id),
            ("account", &self.account),
            ("tool", &self.tool),
            ("canonical_target", &self.canonical_target),
        ] {
            nonempty(text, field, MAX_ID_BYTES)?;
        }
        if self.goal_version == 0 {
            return Err(CoreError::Invalid {
                field: "goal_version",
                reason: "must be positive",
            });
        }
        if !self.arguments.is_object() {
            return Err(CoreError::Invalid {
                field: "arguments",
                reason: "tool arguments must be a JSON object",
            });
        }
        validate_json(&self.arguments)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldConstraint {
    pub json_pointer: String,
    pub expected_value: Value,
    pub source_id: String,
}

/// Load only from broker-owned configuration/native authorization flow. Its
/// Deserialize implementation does not authenticate its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrant {
    pub id: String,
    pub revision: u64,
    pub policy_revision: u64,
    pub goal_id: String,
    pub goal_version: u64,
    pub account: String,
    pub tool: String,
    pub effect: EffectKind,
    pub canonical_target: String,
    pub expires_at_ms: u64,
    pub revoked: bool,
    pub requires_approval: bool,
    pub max_data_class: DataClass,
    pub max_payload_bytes: usize,
    pub allowed_destinations: Vec<String>,
    pub field_constraints: Vec<FieldConstraint>,
}

impl CapabilityGrant {
    pub fn validate(&self) -> Result<(), CoreError> {
        for (field, text) in [
            ("grant_id", &self.id),
            ("goal_id", &self.goal_id),
            ("account", &self.account),
            ("tool", &self.tool),
            ("canonical_target", &self.canonical_target),
        ] {
            nonempty(text, field, MAX_ID_BYTES)?;
        }
        if self.revision == 0
            || self.policy_revision == 0
            || self.goal_version == 0
            || self.expires_at_ms == 0
        {
            return Err(CoreError::Invalid {
                field: "grant_revision",
                reason: "versions must be positive",
            });
        }
        if self.max_payload_bytes == 0 || self.max_payload_bytes > MAX_JSON_BYTES {
            return Err(CoreError::SizeLimit {
                field: "max_payload_bytes",
            });
        }
        if self.allowed_destinations.len() > MAX_FIELDS || self.field_constraints.len() > MAX_FIELDS
        {
            return Err(CoreError::SizeLimit {
                field: "grant_fields",
            });
        }
        for destination in &self.allowed_destinations {
            nonempty(destination, "destination", MAX_ID_BYTES)?;
        }
        let mut fields = std::collections::BTreeSet::new();
        for constraint in &self.field_constraints {
            validate_pointer(&constraint.json_pointer)?;
            nonempty(&constraint.source_id, "source_id", MAX_ID_BYTES)?;
            validate_json(&constraint.expected_value)?;
            if !fields.insert(&constraint.json_pointer) {
                return Err(CoreError::Invalid {
                    field: "field_constraints",
                    reason: "duplicate field",
                });
            }
        }
        Ok(())
    }

    /// Exact-target child grants can narrow but cannot expand the parent scope.
    /// This does not mint an identity/token or perform authentication.
    pub fn is_subset_of(&self, parent: &Self) -> Result<bool, CoreError> {
        self.validate()?;
        parent.validate()?;
        Ok(!self.revoked
            && !parent.revoked
            && self.goal_id == parent.goal_id
            && self.goal_version == parent.goal_version
            && self.policy_revision == parent.policy_revision
            && self.account == parent.account
            && self.tool == parent.tool
            && self.effect == parent.effect
            && self.canonical_target == parent.canonical_target
            && self.expires_at_ms <= parent.expires_at_ms
            && self.max_data_class <= parent.max_data_class
            && self.max_payload_bytes <= parent.max_payload_bytes
            && (!parent.requires_approval || self.requires_approval)
            && self
                .allowed_destinations
                .iter()
                .all(|x| parent.allowed_destinations.contains(x))
            && parent
                .field_constraints
                .iter()
                .all(|x| self.field_constraints.contains(x)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustedDataUse {
    source_id: String,
    classification: DataClass,
    allowed_destinations: Vec<String>,
}

impl TrustedDataUse {
    /// The caller supplies broker/ingestion-owned metadata. The constructor does
    /// not verify real-world truth or classify a model-generated string.
    pub fn new(
        source_id: String,
        classification: DataClass,
        allowed_destinations: Vec<String>,
    ) -> Result<Self, CoreError> {
        nonempty(&source_id, "source_id", MAX_ID_BYTES)?;
        if allowed_destinations.len() > MAX_FIELDS {
            return Err(CoreError::SizeLimit {
                field: "data_destinations",
            });
        }
        for destination in &allowed_destinations {
            nonempty(destination, "destination", MAX_ID_BYTES)?;
        }
        Ok(Self {
            source_id,
            classification,
            allowed_destinations,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldEvidence {
    json_pointer: String,
    source_id: String,
}

impl FieldEvidence {
    pub fn new(json_pointer: String, source_id: String) -> Result<Self, CoreError> {
        validate_pointer(&json_pointer)?;
        nonempty(&source_id, "source_id", MAX_ID_BYTES)?;
        Ok(Self {
            json_pointer,
            source_id,
        })
    }
}

/// Origin labels are kept separate from the untrusted action proposal. No
/// Deserialize implementation is supplied for this trusted context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ActionContext {
    pub data: Vec<TrustedDataUse>,
    pub field_evidence: Vec<FieldEvidence>,
}

impl ActionContext {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.data.len() > MAX_REFS || self.field_evidence.len() > MAX_FIELDS {
            return Err(CoreError::SizeLimit {
                field: "action_context",
            });
        }
        let mut unique = std::collections::BTreeSet::new();
        for field in &self.field_evidence {
            if !unique.insert(&field.json_pointer) {
                return Err(CoreError::Invalid {
                    field: "field_evidence",
                    reason: "duplicate field",
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Allow,
    Deny,
    NeedsInput,
    NeedsApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    pub verdict: Verdict,
    pub reason: &'static str,
}

impl Assessment {
    fn new(verdict: Verdict, reason: &'static str) -> Self {
        Self { verdict, reason }
    }
}

/// A binding for an already authenticated native confirmation. This is not a
/// signed token/authentication mechanism. It intentionally cannot be populated
/// from arbitrary JSON, and must be rechecked immediately before the effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovalBinding {
    fingerprint: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

impl ApprovalBinding {
    pub fn from_trusted_confirmation(
        action: &ActionProposal,
        grant: &CapabilityGrant,
        context: &ActionContext,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Result<Self, CoreError> {
        if issued_at_ms >= expires_at_ms || expires_at_ms > grant.expires_at_ms {
            return Err(CoreError::Invalid {
                field: "approval_expiry",
                reason: "invalid window or beyond grant expiry",
            });
        }
        Ok(Self {
            fingerprint: binding_fingerprint(action, grant, context)?,
            issued_at_ms,
            expires_at_ms,
        })
    }

    pub fn valid_for(
        &self,
        action: &ActionProposal,
        grant: &CapabilityGrant,
        context: &ActionContext,
        now_ms: u64,
    ) -> Result<bool, CoreError> {
        Ok(now_ms >= self.issued_at_ms
            && now_ms < self.expires_at_ms
            && !grant.revoked
            && now_ms < grant.expires_at_ms
            && self.fingerprint == binding_fingerprint(action, grant, context)?)
    }
}

pub fn assess_action(
    action: &ActionProposal,
    grant: &CapabilityGrant,
    context: &ActionContext,
    approval: Option<&ApprovalBinding>,
    now_ms: u64,
) -> Result<Assessment, CoreError> {
    action.validate()?;
    grant.validate()?;
    context.validate()?;
    if grant.revoked || now_ms >= grant.expires_at_ms {
        return Ok(Assessment::new(Verdict::Deny, "grant_expired_or_revoked"));
    }
    if action.goal_id != grant.goal_id
        || action.goal_version != grant.goal_version
        || action.account != grant.account
        || action.tool != grant.tool
        || action.effect != grant.effect
        || action.canonical_target != grant.canonical_target
    {
        return Ok(Assessment::new(Verdict::Deny, "outside_capability_scope"));
    }
    if canonical_size(&action.arguments)? > grant.max_payload_bytes {
        return Ok(Assessment::new(Verdict::Deny, "payload_budget_exceeded"));
    }
    for field in &grant.field_constraints {
        if action.arguments.pointer(&field.json_pointer) != Some(&field.expected_value)
            || !context
                .field_evidence
                .iter()
                .any(|e| e.json_pointer == field.json_pointer && e.source_id == field.source_id)
        {
            return Ok(Assessment::new(
                Verdict::NeedsInput,
                "unconfirmed_critical_field",
            ));
        }
    }
    for data in &context.data {
        if data.classification > grant.max_data_class {
            return Ok(Assessment::new(Verdict::Deny, "classification_not_granted"));
        }
        if action.effect == EffectKind::LocalWrite
            && !data.allowed_destinations.contains(&action.canonical_target)
        {
            return Ok(Assessment::new(Verdict::Deny, "source_destination_denied"));
        }
    }
    if action.effect.is_egress() {
        if context.data.is_empty() {
            return Ok(Assessment::new(Verdict::Deny, "missing_data_provenance"));
        }
        if !grant
            .allowed_destinations
            .contains(&action.canonical_target)
        {
            return Ok(Assessment::new(Verdict::Deny, "destination_not_granted"));
        }
        for data in &context.data {
            if data.classification == DataClass::Secret {
                return Ok(Assessment::new(Verdict::Deny, "secret_egress_denied"));
            }
            if !data.allowed_destinations.contains(&action.canonical_target) {
                return Ok(Assessment::new(Verdict::Deny, "source_destination_denied"));
            }
        }
    }
    if grant.requires_approval {
        let valid = match approval {
            Some(binding) => binding.valid_for(action, grant, context, now_ms)?,
            None => false,
        };
        if !valid {
            return Ok(Assessment::new(
                Verdict::NeedsApproval,
                "exact_confirmation_required",
            ));
        }
    }
    Ok(Assessment::new(Verdict::Allow, "within_capability"))
}

pub fn action_fingerprint(action: &ActionProposal) -> Result<String, CoreError> {
    action.validate()?;
    canonical_sha256(&serde_json::to_value(action)?)
}

fn binding_fingerprint(
    action: &ActionProposal,
    grant: &CapabilityGrant,
    context: &ActionContext,
) -> Result<String, CoreError> {
    action.validate()?;
    grant.validate()?;
    context.validate()?;
    canonical_sha256(&serde_json::json!({"action": action, "grant": grant, "context": context}))
}

fn validate_pointer(pointer: &str) -> Result<(), CoreError> {
    if pointer.len() > MAX_ID_BYTES || (!pointer.is_empty() && !pointer.starts_with('/')) {
        return Err(CoreError::Invalid {
            field: "json_pointer",
            reason: "invalid or oversized JSON pointer",
        });
    }
    Ok(())
}

fn validate_json(value: &Value) -> Result<(), CoreError> {
    fn visit(value: &Value, depth: usize, count: &mut usize) -> Result<(), CoreError> {
        *count += 1;
        if depth > MAX_JSON_DEPTH || *count > MAX_JSON_NODES {
            return Err(CoreError::SizeLimit {
                field: "JSON_structure",
            });
        }
        match value {
            Value::String(s) if s.len() > MAX_TEXT_BYTES => {
                return Err(CoreError::SizeLimit {
                    field: "JSON_string",
                });
            }
            Value::Array(values) => {
                for v in values {
                    visit(v, depth + 1, count)?;
                }
            }
            Value::Object(values) => {
                for (key, v) in values {
                    if key.len() > MAX_ID_BYTES {
                        return Err(CoreError::SizeLimit { field: "JSON_key" });
                    }
                    visit(v, depth + 1, count)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, 0, &mut 0)
}

struct Canonical<'a>(&'a Value);

impl Serialize for Canonical<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(values) => {
                let sorted: BTreeMap<&str, &Value> =
                    values.iter().map(|(k, v)| (k.as_str(), v)).collect();
                let mut map = serializer.serialize_map(Some(sorted.len()))?;
                for (key, value) in sorted {
                    map.serialize_entry(key, &Canonical(value))?;
                }
                map.end()
            }
            Value::Array(values) => {
                let mut seq = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    seq.serialize_element(&Canonical(value))?;
                }
                seq.end()
            }
            value => value.serialize(serializer),
        }
    }
}

struct DigestWriter {
    digest: Sha256,
    size: usize,
}

impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let size = self
            .size
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("canonical size overflow"))?;
        if size > MAX_JSON_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "canonical message too large",
            ));
        }
        self.digest.update(bytes);
        self.size = size;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn canonical_writer(value: &Value) -> Result<DigestWriter, CoreError> {
    validate_json(value)?;
    let mut writer = DigestWriter {
        digest: Sha256::new(),
        size: 0,
    };
    serde_json::to_writer(&mut writer, &Canonical(value))?;
    Ok(writer)
}

/// Deterministic exact-content binding. Object keys are sorted even if another
/// crate enables serde_json's preserve_order feature. This is not a signature,
/// RFC8785 numeric normalization or an authentication token; 1 and1.0 may bind
/// differently. Canonical bytes stream directly into SHA256 without a full copy.
pub fn canonical_sha256(value: &Value) -> Result<String, CoreError> {
    Ok(format!("{:x}", canonical_writer(value)?.digest.finalize()))
}

fn canonical_size(value: &Value) -> Result<usize, CoreError> {
    Ok(canonical_writer(value)?.size)
}
