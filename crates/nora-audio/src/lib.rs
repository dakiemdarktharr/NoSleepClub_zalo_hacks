//! Bounded audio primitives; speech models and microphone access are opt-in.
//!
//! No model weights are bundled or downloaded. Native inference remains in
//! ONNX Runtime/whisper.cpp; first-party framing, endpointing and playback are Rust.
//! VAD is neither speaker authentication nor acoustic echo cancellation.

mod endpoint;
mod pcm;
mod playback;
mod resample;

#[cfg(feature = "capture")]
pub mod capture;
#[cfg(feature = "onnx")]
pub mod silero;
#[cfg(feature = "whisper")]
pub mod whisper;

pub use endpoint::{EndReason, EndpointConfig, EndpointDetector, EndpointEvent};
pub use pcm::{FrameAssembler, MAX_BUFFER_SAMPLES, PcmRing, downmix_interleaved};
pub use playback::PlaybackQueue;
pub use resample::MonoResampler;
#[cfg(feature = "onnx")]
pub use silero::SileroVad;
#[cfg(feature = "whisper")]
pub use whisper::{Transcript, TranscriptSegment, WhisperEngine};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AudioError {
    #[error("invalid audio configuration: {0}")]
    Configuration(&'static str),
    #[error("audio contains a non-finite sample")]
    NonFiniteSample,
    #[error("audio buffer full: {requested} samples requested, {available} available")]
    BufferFull { requested: usize, available: usize },
    #[error("output buffer needs {needed} samples, received {provided}")]
    OutputTooSmall { needed: usize, provided: usize },
    #[error("expected {expected} input samples, received {actual}")]
    InputLength { expected: usize, actual: usize },
    #[error("probability must be finite and in [0, 1]")]
    InvalidProbability,
    #[error("playback generation is stale")]
    StaleGeneration,
    #[error("playback is closed")]
    PlaybackClosed,
    #[error("playback generation exhausted")]
    GenerationExhausted,
    #[error("audio allocation failed")]
    Allocation,
    #[error("native audio backend: {0}")]
    Backend(String),
}

pub(crate) fn finite(samples: &[f32]) -> Result<(), AudioError> {
    if samples.iter().all(|sample| sample.is_finite()) {
        Ok(())
    } else {
        Err(AudioError::NonFiniteSample)
    }
}
