//! Rust control plane for the VieNeu-TTS v3 Turbo CPU graphs.
//!
//! This is a bounded, local-artifact host, not a neural-engine rewrite. It does
//! not download or execute repository code, silently switch precision, or clone
//! voices. The graph pack and dictionary must come from a verified manifest.
//! First-party algorithms derived from VieNeu are identified in `NOTICE`.

mod assets;
mod config;
mod heads;
mod prompt;
mod sampling;
mod stream;

#[cfg(feature = "g2p")]
pub mod g2p;
#[cfg(feature = "onnx")]
pub mod onnx;

pub use assets::ModelPack;
pub use config::{Limits, ModelConfig};
pub use heads::{Heads, SpeakerProjection};
pub use prompt::{Prompt, VoicePreset, max_expected_frames, syllable_count};
pub use sampling::{RepetitionHistory, Sampler, SamplingConfig};
pub use stream::{CancellationToken, CodecStateSpec, StateDtype, StateSpec, StreamLayout};

use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O failure at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("invalid model/config/input: {0}")]
    Invalid(String),
    #[error("resource limit exceeded: {0}")]
    Limit(String),
    #[error("required model asset missing: {0}")]
    Missing(String),
    #[error("synthesis cancelled")]
    Cancelled,
    #[error("model runtime: {0}")]
    Runtime(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Opt into serial tokenization for a single-user voice runtime. This changes
/// tokenizers' process-global parallelism policy, so call once at application
/// startup before workers/models. Libraries do not call it implicitly. It does
/// not mutate the environment or alter ORT's independently configured threads.
pub fn configure_single_user_runtime() {
    tokenizers::utils::parallelism::set_parallelism(false);
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

fn io_error(path: impl Into<PathBuf>, source: io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}

fn checked_elements(shape: &[usize]) -> Result<usize> {
    shape.iter().try_fold(1usize, |n, &dim| {
        n.checked_mul(dim)
            .ok_or_else(|| Error::Limit("tensor shape overflow".into()))
    })
}
