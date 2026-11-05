//! Contest-only authority boundary. Dataset labels, environment state and cloud
//! credentials are deliberately absent from the inference types.

pub mod actor_v2;
pub mod aliases;
pub mod assets;
pub(crate) mod async_transport;
pub mod bridge;
pub mod clarification;
pub mod cohort;
#[cfg(feature = "semantic-retrieval")]
pub mod embedding;
pub mod grounding;
pub mod local;
pub mod native_tools;
pub mod plan;
pub mod plan_guard;
pub mod planning;
pub mod progressive;
pub mod protocol;
pub mod retrieval;
pub mod routing;
pub mod runner;
#[cfg(feature = "semantic-retrieval")]
pub mod semantic_retrieval;
#[cfg(feature = "whisper")]
pub mod speech;
#[cfg(feature = "whisper")]
pub mod speech_gpu;
pub mod training_bindings;
pub mod training_data;
pub mod xlam;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const MAX_JSON_BYTES: usize = 2_097_152;
pub const MAX_TOOLS: usize = 256;
pub const MAX_TOOL_CALLS: usize = 32;
pub const MAX_MODEL_TURNS: usize = 12;
pub const MAX_CONTEXT_TOKENS: u32 = 16_384;
pub const SAMPLE_DEADLINE_MS: u64 = 30_000;
