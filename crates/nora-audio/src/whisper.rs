//! Bounded CPU whisper.cpp host through the maintained whisper-rs bindings.
//! Weights are external; this module never downloads or executes repository code.

use crate::{AudioError, MAX_BUFFER_SAMPLES, finite};
use serde::{Deserialize, Serialize};
use std::path::Path;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

const MAX_TRANSCRIPT_BYTES: usize = 65_536;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub sample_rate: u32,
    pub language: String,
    pub text: String,
    pub segments: Vec<TranscriptSegment>,
}

pub struct WhisperEngine {
    state: WhisperState,
    max_samples: usize,
    threads: i32,
}

fn backend(error: impl std::fmt::Display) -> AudioError {
    AudioError::Backend(error.to_string())
}

fn validate_request(samples: &[f32], max_samples: usize, language: &str) -> Result<(), AudioError> {
    if samples.is_empty() || samples.len() > max_samples {
        return Err(AudioError::Configuration(
            "ASR input exceeds configured duration",
        ));
    }
    finite(samples)?;
    // Validate before upstream CString helpers: embedded NUL is not a language.
    if !(2..=3).contains(&language.len())
        || !language.bytes().all(|byte| byte.is_ascii_lowercase())
        || whisper_rs::get_lang_id(language).is_none()
    {
        return Err(AudioError::Configuration("unsupported ASR language"));
    }
    Ok(())
}

impl WhisperEngine {
    /// CPU is explicit here; GPU feature/runtime selection requires its own gate.
    /// max_samples limits each request, not native model memory.
    pub fn load(
        path: impl AsRef<Path>,
        max_samples: usize,
        threads: usize,
    ) -> Result<Self, AudioError> {
        if max_samples == 0 || max_samples > MAX_BUFFER_SAMPLES || !(1..=16).contains(&threads) {
            return Err(AudioError::Configuration(
                "invalid ASR duration/thread budget",
            ));
        }
        // The currently tested x86 native build disables GGML_NATIVE but still
        // uses upstream AVX2/F16C/FMA defaults. Reject older CPUs before FFI.
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if !(std::is_x86_feature_detected!("avx2")
            && std::is_x86_feature_detected!("f16c")
            && std::is_x86_feature_detected!("fma"))
        {
            return Err(AudioError::Configuration(
                "this Whisper CPU profile requires AVX2/F16C/FMA",
            ));
        }
        if !path.as_ref().is_file() {
            return Err(AudioError::Configuration("ASR model file does not exist"));
        }
        let mut config = WhisperContextParameters::default();
        config.use_gpu(false);
        let context = WhisperContext::new_with_params(path, config).map_err(backend)?;
        let state = context.create_state().map_err(backend)?;
        Ok(Self {
            state,
            max_samples,
            threads: threads as i32,
        })
    }

    /// Input must be mono float PCM at16kHz. No translation or previous prompt
    /// carry-over is enabled; a previous goal cannot supply a hidden ASR prompt.
    pub fn transcribe_16khz(
        &mut self,
        samples: &[f32],
        language: &str,
    ) -> Result<Transcript, AudioError> {
        validate_request(samples, self.max_samples, language)?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(self.threads);
        params.set_language(Some(language));
        params.set_translate(false);
        params.set_no_context(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        self.state.full(params, samples).map_err(backend)?;
        let mut text = String::new();
        let mut segments = Vec::new();
        for segment in self.state.as_iter() {
            let segment_text = segment.to_str().map_err(backend)?;
            if text
                .len()
                .checked_add(segment_text.len())
                .is_none_or(|n| n > MAX_TRANSCRIPT_BYTES)
            {
                return Err(AudioError::Configuration("ASR transcript exceeds bound"));
            }
            let start_ms = u64::try_from(segment.start_timestamp())
                .map_err(backend)?
                .checked_mul(10)
                .ok_or(AudioError::Configuration("ASR timestamp overflow"))?;
            let end_ms = u64::try_from(segment.end_timestamp())
                .map_err(backend)?
                .checked_mul(10)
                .ok_or(AudioError::Configuration("ASR timestamp overflow"))?;
            if end_ms < start_ms {
                return Err(AudioError::Configuration(
                    "ASR returned reversed timestamps",
                ));
            }
            text.push_str(segment_text);
            segments.push(TranscriptSegment {
                start_ms,
                end_ms,
                text: segment_text.to_owned(),
            });
        }
        Ok(Transcript {
            sample_rate: 16_000,
            language: language.to_owned(),
            text,
            segments,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_request_before_native_inference() {
        assert!(validate_request(&[0.0; 512], 512, "vi").is_ok());
        for language in ["", "vi\0", "VI", "zz", "vietnamese"] {
            assert!(validate_request(&[0.0], 512, language).is_err());
        }
        assert!(validate_request(&[], 512, "vi").is_err());
        assert!(validate_request(&[0.0; 513], 512, "vi").is_err());
        assert_eq!(
            validate_request(&[f32::NAN], 512, "vi"),
            Err(AudioError::NonFiniteSample)
        );
    }
    #[test]
    fn rejects_invalid_model_budget_without_loading_model() {
        assert!(WhisperEngine::load("missing-model.bin", 0, 1).is_err());
        assert!(WhisperEngine::load("missing-model.bin", 512, 17).is_err());
        assert!(WhisperEngine::load("missing-model.bin", 512, 1).is_err());
    }
}
