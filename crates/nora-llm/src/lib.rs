//! Bounded OpenAI Responses transport, not an autonomous tool executor.
//!
//! Configuration is explicitly selected and never installed in process-wide
//! environment variables. Requests stay on the configured HTTPS origin; proxy,
//! redirects, protocol retries and TLS key logging are disabled. This cannot
//! prevent disclosure deliberately authorized by an application: the caller
//! still owns consent, tool authority, provenance and per-goal accounting.

mod client;
mod config;
mod types;

pub use client::ResponsesClient;
pub use config::{ApiConfig, ConfigSummary, GoalLimits};
pub use types::{IncompleteReason, ResponseStatus, ToolCall, TurnRequest, TurnResponse, Usage};

pub const MAX_CONFIG_BYTES: usize = 65_536;
pub const MAX_REQUEST_BYTES: usize = 1_048_576;
pub const MAX_RESPONSE_BYTES: usize = 2_097_152;
pub const MAX_OUTPUT_TEXT_BYTES: usize = 65_536;
pub const MAX_TOOL_ARGUMENT_BYTES: usize = 65_536;
pub const MAX_INPUT_ITEMS: usize = 256;
pub const MAX_OUTPUT_ITEMS: usize = 128;
pub const MAX_TOOLS: usize = 32;
pub const MAX_TOOL_CALLS: usize = 8;
pub const MAX_OUTPUT_TOKENS: u32 = 8192;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiError {
    #[error("local LLM configuration could not be read")]
    ConfigRead,
    #[error("invalid LLM configuration: {0}")]
    Config(&'static str),
    #[error("NORA_LLM_API_KEY is empty; save the key in the selected local configuration")]
    MissingApiKey,
    #[error("NORA_LLM_MODEL is empty; select an explicit model")]
    MissingModel,
    #[error("LLM request blocked because it contains the configured API credential")]
    CredentialDisclosure,
    #[error("LLM request cancelled; server-side work may already have incurred usage")]
    Cancelled,
    #[error("LLM request deadline exceeded; no automatic retry was performed")]
    Timeout,
    #[error("LLM HTTP request failed with status {status}; no automatic retry was performed")]
    Http {
        status: u16,
        retry_after_seconds: Option<u64>,
    },
    #[error("LLM connection failed; provider details are intentionally not included")]
    Transport,
    #[error("LLM protocol error: {0}")]
    Protocol(&'static str),
    #[error("LLM size or resource limit: {0}")]
    Limit(&'static str),
    #[error("LLM client is already serving another turn")]
    Busy,
    #[error("LLM worker runtime is unavailable")]
    Runtime,
}

pub type Result<T> = std::result::Result<T, ApiError>;
