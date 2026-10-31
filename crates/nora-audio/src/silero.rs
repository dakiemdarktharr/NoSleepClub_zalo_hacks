//! Silero's explicit recurrent-state ONNX protocol, not Torch or hub code.
//!
//! Protocol source: snakers4/silero-vad, src/silero_vad/utils_vad.py,
//! OnnxWrapper (MIT). This host is original Rust code mirroring that interface.
//! The supported signature is input/state/sr -> output/stateN at 16 kHz.

use crate::{AudioError, finite};
use ort::{
    environment::Environment,
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use std::path::Path;

const FRAME: usize = 512;
const CONTEXT: usize = 64;
const STATE: usize = 2 * 128;

/// One single-stream CPU Silero session. One Rust host owns recurrent state.
/// Reuse a shared ORT Environment across VAD/TTS; load only trusted model assets.
pub struct SileroVad {
    session: Session,
    state: [f32; STATE],
    context: [f32; CONTEXT],
    input: [f32; FRAME + CONTEXT],
}

fn backend(error: impl std::fmt::Display) -> AudioError {
    AudioError::Backend(error.to_string())
}

impl SileroVad {
    pub const SAMPLE_RATE: u32 = 16_000;
    pub const FRAME_SAMPLES: usize = FRAME;

    pub fn load(
        environment: &Environment,
        path: impl AsRef<Path>,
        threads: usize,
    ) -> Result<Self, AudioError> {
        if !(1..=8).contains(&threads) {
            return Err(AudioError::Configuration("VAD threads must be in 1..=8"));
        }
        let session = Session::builder(environment)
            .map_err(backend)?
            .with_intra_threads(threads)
            .map_err(backend)?
            .with_inter_threads(1)
            .map_err(backend)?
            .with_parallel_execution(false)
            .map_err(backend)?
            .with_intra_op_spinning(false)
            .map_err(backend)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(backend)?
            .commit_from_file(path)
            .map_err(backend)?;
        Ok(Self {
            session,
            state: [0.0; STATE],
            context: [0.0; CONTEXT],
            input: [0.0; FRAME + CONTEXT],
        })
    }

    pub fn reset(&mut self) {
        self.state.fill(0.0);
        self.context.fill(0.0);
        self.input.fill(0.0);
    }

    pub fn probability(&mut self, frame: &[f32]) -> Result<f32, AudioError> {
        if frame.len() != FRAME {
            return Err(AudioError::InputLength {
                expected: FRAME,
                actual: frame.len(),
            });
        }
        finite(frame)?;
        self.input[..CONTEXT].copy_from_slice(&self.context);
        self.input[CONTEXT..].copy_from_slice(frame);
        let sr = [16_000i64];
        let output=self.session.run(ort::inputs![
            "input"=>TensorRef::from_array_view(([1usize,FRAME+CONTEXT],&self.input[..])).map_err(backend)?,
            "state"=>TensorRef::from_array_view(([2usize,1,128],&self.state[..])).map_err(backend)?,
            "sr"=>TensorRef::from_array_view(([] as [usize;0],&sr[..])).map_err(backend)?,
        ]).map_err(backend)?;
        let (prob_shape, prob) = output
            .get("output")
            .ok_or_else(|| backend("Silero graph lacks output"))?
            .try_extract_tensor::<f32>()
            .map_err(backend)?;
        if prob.len() != 1 || !prob_shape.iter().all(|dimension| *dimension == 1) {
            return Err(backend("Silero probability shape mismatch"));
        }
        let probability = prob[0];
        if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
            return Err(AudioError::InvalidProbability);
        }
        let (state_shape, state) = output
            .get("stateN")
            .ok_or_else(|| backend("Silero graph lacks stateN"))?
            .try_extract_tensor::<f32>()
            .map_err(backend)?;
        if state_shape.as_ref() != [2i64, 1, 128] || state.len() != STATE {
            return Err(backend("Silero recurrent-state shape mismatch"));
        }
        finite(state)?;
        self.state.copy_from_slice(state);
        self.context.copy_from_slice(&frame[FRAME - CONTEXT..]);
        Ok(probability)
    }
}
