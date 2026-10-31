use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::{Deserialize, Serialize};

use crate::{Error, Limits, Result, checked_elements, invalid};

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransformerOffset {
    pub input_name: String,
    pub output_name: String,
    pub shape: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttentionCache {
    pub offset_input_name: String,
    pub offset_output_name: String,
    pub offset_shape: Vec<usize>,
    pub cached_keys_input_name: String,
    pub cached_keys_output_name: String,
    pub cached_values_input_name: String,
    pub cached_values_output_name: String,
    pub cache_shape: Vec<usize>,
    pub cached_positions_input_name: String,
    pub cached_positions_output_name: String,
    pub positions_shape: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamLayout {
    pub transformer_offsets: Vec<TransformerOffset>,
    pub attention_caches: Vec<AttentionCache>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateDtype {
    Float32,
    Int32,
}

#[derive(Debug, Clone)]
pub struct StateSpec {
    pub input_name: String,
    pub output_name: String,
    pub shape: Vec<usize>,
    pub dtype: StateDtype,
    /// Cached positions MUST begin at -1; offsets/keys/values begin at zero.
    pub initial_i32: i32,
}

pub type CodecStateSpec = Vec<StateSpec>;

impl StreamLayout {
    pub fn specs(&self) -> CodecStateSpec {
        let mut specs =
            Vec::with_capacity(self.transformer_offsets.len() + 4 * self.attention_caches.len());
        for t in &self.transformer_offsets {
            specs.push(StateSpec {
                input_name: t.input_name.clone(),
                output_name: t.output_name.clone(),
                shape: t.shape.clone(),
                dtype: StateDtype::Int32,
                initial_i32: 0,
            });
        }
        for a in &self.attention_caches {
            for (input, output, shape, dtype, init) in [
                (
                    &a.offset_input_name,
                    &a.offset_output_name,
                    &a.offset_shape,
                    StateDtype::Int32,
                    0,
                ),
                (
                    &a.cached_keys_input_name,
                    &a.cached_keys_output_name,
                    &a.cache_shape,
                    StateDtype::Float32,
                    0,
                ),
                (
                    &a.cached_values_input_name,
                    &a.cached_values_output_name,
                    &a.cache_shape,
                    StateDtype::Float32,
                    0,
                ),
                (
                    &a.cached_positions_input_name,
                    &a.cached_positions_output_name,
                    &a.positions_shape,
                    StateDtype::Int32,
                    -1,
                ),
            ] {
                specs.push(StateSpec {
                    input_name: input.clone(),
                    output_name: output.clone(),
                    shape: shape.clone(),
                    dtype,
                    initial_i32: init,
                });
            }
        }
        specs
    }

    pub fn validate(&self, limits: &Limits) -> Result<()> {
        if self.transformer_offsets.len() > 64 || self.attention_caches.len() > 64 {
            return Err(Error::Limit("stream state field count".into()));
        }
        let mut inputs = HashSet::from(["audio_codes".to_owned(), "audio_code_lengths".to_owned()]);
        let mut outputs = HashSet::from(["audio".to_owned(), "audio_lengths".to_owned()]);
        let mut bytes = 0usize;
        for s in self.specs() {
            if s.input_name.is_empty()
                || s.output_name.is_empty()
                || s.input_name.len() > 128
                || s.output_name.len() > 128
                || !inputs.insert(s.input_name)
                || !outputs.insert(s.output_name)
                || s.shape.is_empty()
                || s.shape.len() > 8
                || s.shape.contains(&0)
            {
                return Err(invalid("invalid/duplicate stream state layout"));
            }
            let n = checked_elements(&s.shape)?
                .checked_mul(4)
                .ok_or_else(|| Error::Limit("stream state size".into()))?;
            bytes = bytes
                .checked_add(n)
                .ok_or_else(|| Error::Limit("stream state size".into()))?;
            if bytes > limits.max_state_bytes {
                return Err(Error::Limit("codec stream state budget".into()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_clone_shares_generation_without_resetting_old_work() {
        let old = CancellationToken::default();
        let worker = old.clone();
        old.cancel();
        assert!(matches!(worker.check(), Err(Error::Cancelled)));
        assert!(CancellationToken::default().check().is_ok());
    }
    #[test]
    fn duplicate_or_excessive_state_is_rejected() {
        let t = TransformerOffset {
            input_name: "offset".into(),
            output_name: "next".into(),
            shape: vec![1],
        };
        let mut l = StreamLayout {
            transformer_offsets: vec![t.clone(), t],
            attention_caches: vec![],
        };
        assert!(l.validate(&Limits::default()).is_err());
        l.transformer_offsets.truncate(1);
        l.transformer_offsets[0].shape = vec![usize::MAX, 2];
        assert!(l.validate(&Limits::default()).is_err());
    }

    #[test]
    fn codec_positions_start_negative_and_budget_covers_all_state() {
        let layout:StreamLayout=serde_json::from_value(serde_json::json!({
            "transformer_offsets":[],"attention_caches":[{
                "offset_input_name":"off","offset_output_name":"off_out","offset_shape":[1],
                "cached_keys_input_name":"k","cached_keys_output_name":"k_out",
                "cached_values_input_name":"v","cached_values_output_name":"v_out","cache_shape":[1,2,4,2],
                "cached_positions_input_name":"pos","cached_positions_output_name":"pos_out","positions_shape":[1,4]
            }]
        })).unwrap();
        layout.validate(&Limits::default()).unwrap();
        let specs = layout.specs();
        let positions = specs.iter().find(|s| s.input_name == "pos").unwrap();
        assert_eq!(positions.initial_i32, -1);
        assert_eq!(positions.dtype, StateDtype::Int32);
        assert!(
            specs
                .iter()
                .filter(|s| s.input_name != "pos")
                .all(|s| s.initial_i32 == 0)
        );
        let limits = Limits {
            max_state_bytes: 1,
            ..Default::default()
        };
        assert!(layout.validate(&limits).is_err());
    }
}
