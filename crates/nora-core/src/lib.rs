//! Pure policy and admission primitives for NORA.
//!
//! No microphone, model, app automation, OS sandbox or authentication is provided
//! here. Broker callers must authenticate event/configuration sources and meter
//! actual process resources. Public JSON types are not authority by themselves.

pub mod notifications;
pub mod policy;
pub mod resources;
pub mod routing;

use serde::de::DeserializeOwned;

/// Maximum encoded message accepted by [`from_json_bounded`]. Call this before
/// deserializing untrusted JSON; validation after deserialization cannot undo an
/// oversized allocation already performed by a generic parser.
pub const MAX_JSON_BYTES: usize = 256 * 1024;
pub const MAX_ID_BYTES: usize = 256;
pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_REFS: usize = 1024;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{field}: {reason}")]
    Invalid {
        field: &'static str,
        reason: &'static str,
    },
    #[error("{field} exceeds its configured size limit")]
    SizeLimit { field: &'static str },
    #[error("resource arithmetic overflow")]
    Overflow,
    #[error("resource budget exceeded: {0}")]
    BudgetExceeded(&'static str),
    #[error("reservation already exists")]
    DuplicateReservation,
    #[error("unknown reservation")]
    UnknownReservation,
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn from_json_bounded<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CoreError> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(CoreError::SizeLimit { field: "message" });
    }
    Ok(serde_json::from_slice(bytes)?)
}

pub(crate) fn nonempty(value: &str, field: &'static str, limit: usize) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        return Err(CoreError::Invalid {
            field,
            reason: "must be nonempty text",
        });
    }
    if value.len() > limit {
        return Err(CoreError::SizeLimit { field });
    }
    Ok(())
}

pub(crate) fn timestamp(value: f64, field: &'static str) -> Result<(), CoreError> {
    if !value.is_finite() || value < 0.0 {
        return Err(CoreError::Invalid {
            field,
            reason: "must be finite and nonnegative",
        });
    }
    Ok(())
}

pub(crate) fn refs(values: &[String], field: &'static str) -> Result<(), CoreError> {
    if values.len() > MAX_REFS {
        return Err(CoreError::SizeLimit { field });
    }
    for value in values {
        nonempty(value, field, MAX_ID_BYTES)?;
    }
    Ok(())
}
