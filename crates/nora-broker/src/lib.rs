//! Single-owner local capability broker. This library is not an OS sandbox.
//!
//! Root/app grants and `approve` are trusted UI operations, never model tools.
//! Model input may create only `ToolAction`; it cannot deserialize an approval
//! token or authorize its own grant. Callers must keep these entry points apart.

mod arithmetic;
mod files;
mod store;

use cap_std::fs::Dir;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, path::PathBuf};
use thiserror::Error;

pub use files::{MAX_FILE_BYTES, MAX_LIST_ENTRIES};
pub use store::Broker;

pub type Result<T> = std::result::Result<T, BrokerError>;

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("local broker I/O operation failed")]
    Io(#[from] std::io::Error),
    #[error("local broker database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("invalid broker JSON")]
    Json(#[from] serde_json::Error),
    #[error("broker state belongs to another application or is corrupt")]
    ForeignState,
    #[error("another trusted supervisor owns this broker state")]
    AlreadyOwned,
    #[error("invalid {0}")]
    Invalid(&'static str),
    #[error("broker limit exceeded: {0}")]
    Limit(&'static str),
    #[error("grant is missing, revoked or insufficient")]
    PermissionDenied,
    #[error("this grant does not allow data disclosure to the LLM")]
    DisclosureDenied,
    #[error("a fresh trusted UI approval is required")]
    ApprovalRequired,
    #[error("approval expired, was consumed, or no longer matches this goal/action/grant")]
    ApprovalMismatch,
    #[error("operation was cancelled")]
    Cancelled,
    #[error("target already exists; overwrite is not permitted")]
    TargetExists,
    #[error("external action outcome is uncertain; reconcile before retrying")]
    NeedsReconciliation,
    #[error("bounded calculator expression is invalid")]
    Arithmetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootPermissions {
    pub read: bool,
    pub create_new: bool,
    pub cloud_disclosure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootGrant {
    pub id: String,
    pub label: String,
    /// Trusted UI only. Never put a local-only root path in LLM context.
    pub path: PathBuf,
    pub permissions: RootPermissions,
    pub revision: u64,
    pub revoked: bool,
    pub created_ms: i64,
    pub updated_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppGrant {
    pub id: String,
    pub label: String,
    /// Trusted UI only; tools address an opaque app ID and accept no arguments.
    pub executable: PathBuf,
    pub sha256: String,
    pub revision: u64,
    pub revoked: bool,
    pub created_ms: i64,
    pub updated_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolAction {
    ListFiles {
        root_id: String,
        path: String,
    },
    ReadText {
        root_id: String,
        path: String,
    },
    SearchText {
        root_id: String,
        path: String,
        query: String,
    },
    CreateNote {
        root_id: String,
        path: String,
        content: String,
    },
    OpenApp {
        app_id: String,
    },
    Calculate {
        expression: String,
    },
    CurrentTime,
    CreateReminder {
        text: String,
        due_at_ms: i64,
    },
    ListReminders,
}

impl ToolAction {
    pub fn requires_approval(&self) -> bool {
        matches!(
            self,
            Self::CreateNote { .. } | Self::OpenApp { .. } | Self::CreateReminder { .. }
        )
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ListFiles { root_id, path } => {
                identifier(root_id)?;
                files::relative(path, true)?;
            }
            Self::ReadText { root_id, path } => {
                identifier(root_id)?;
                files::relative(path, false)?;
            }
            Self::SearchText {
                root_id,
                path,
                query,
            } => {
                identifier(root_id)?;
                files::relative(path, false)?;
                bounded_text(query, 256, "search query")?;
            }
            Self::CreateNote {
                root_id,
                path,
                content,
            } => {
                identifier(root_id)?;
                files::relative(path, false)?;
                bounded_text(content, MAX_FILE_BYTES as usize, "note content")?;
                if !path.to_ascii_lowercase().ends_with(".md")
                    && !path.to_ascii_lowercase().ends_with(".txt")
                {
                    return Err(BrokerError::Invalid("note extension (only .md/.txt)"));
                }
            }
            Self::OpenApp { app_id } => identifier(app_id)?,
            Self::Calculate { expression } => bounded_text(expression, 256, "expression")?,
            Self::CreateReminder { text, due_at_ms } => {
                bounded_text(text, 2048, "reminder text")?;
                if *due_at_ms < 0 {
                    return Err(BrokerError::Invalid("reminder timestamp"));
                }
            }
            Self::CurrentTime | Self::ListReminders => {}
        }
        Ok(())
    }
}

/// Created by trusted runtime code, not deserialized from a model tool call.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub goal_id: String,
    pub goal_revision: u64,
    pub session_id: String,
    pub now_ms: i64,
    /// True whenever any output/metadata is about to enter LLM context.
    pub disclose_to_llm: bool,
    pub cancelled: bool,
}

impl ExecutionContext {
    fn validate(&self) -> Result<()> {
        identifier(&self.goal_id)?;
        if self.goal_revision == 0 {
            return Err(BrokerError::Invalid("goal revision"));
        }
        identifier(&self.session_id)?;
        if self.now_ms < 0 {
            return Err(BrokerError::Invalid("timestamp"));
        }
        if self.cancelled {
            return Err(BrokerError::Cancelled);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionPreview {
    pub id: String,
    pub goal_id: String,
    pub goal_revision: u64,
    pub session_id: String,
    pub summary: String,
    pub fingerprint: String,
    pub expires_at_ms: i64,
    pub grant_revision: u64,
    pub details: Value,
}

/// In-process proof returned only to the trusted UI approval path. No Clone or
/// Deserialize: model-provided JSON cannot construct this value.
pub struct ApprovalToken {
    preview_id: String,
    nonce: [u8; 32],
}

impl std::fmt::Debug for ApprovalToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApprovalToken([redacted])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Disclosure {
    LocalOnly,
    CloudAllowed,
}

/// Authorization-only result. The trusted launcher must keep this receipt alive
/// until spawn; on Windows its open handle prevents replacing the approved EXE.
/// No process has been launched merely because this value exists.
pub struct ApprovedLaunch {
    pub executable: PathBuf,
    pub expected_sha256: String,
    pub approval_id: String,
    pub(crate) _file: File,
}

impl std::fmt::Debug for ApprovedLaunch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovedLaunch")
            .field("approval_id", &self.approval_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub struct ToolReceipt {
    pub data: Value,
    pub verified: bool,
    pub disclosure: Disclosure,
    pub approval_id: Option<String>,
    #[serde(skip)]
    pub launch: Option<ApprovedLaunch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reminder {
    pub id: String,
    pub goal_id: String,
    pub text: String,
    pub due_at_ms: i64,
    pub dismissed: bool,
    pub cloud_disclosure: bool,
}

struct RootHandle {
    grant: RootGrant,
    dir: Dir,
}
type RootHandles = BTreeMap<String, RootHandle>;

fn identifier(text: &str) -> Result<()> {
    bounded_text(text, 128, "identifier")?;
    if text.chars().any(char::is_whitespace) {
        return Err(BrokerError::Invalid("identifier"));
    }
    Ok(())
}

fn bounded_text(text: &str, max: usize, field: &'static str) -> Result<()> {
    if text.trim().is_empty()
        || text.len() > max
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(BrokerError::Invalid(field));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fresh_id(prefix: &str) -> Result<String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|_| BrokerError::Invalid("system entropy"))?;
    let mut text = String::with_capacity(prefix.len() + 48);
    text.push_str(prefix);
    for b in bytes {
        use std::fmt::Write;
        write!(text, "{b:02x}").expect("String formatting");
    }
    Ok(text)
}

fn fingerprint(
    action: &ToolAction,
    context: &ExecutionContext,
    grant_revision: u64,
) -> Result<String> {
    Ok(digest(&serde_json::to_vec(
        &json!({"version":1,"action":action,"goal_id":context.goal_id,"goal_revision":context.goal_revision,"session_id":context.session_id,"grant_revision":grant_revision,"disclose_to_llm":context.disclose_to_llm}),
    )?))
}
