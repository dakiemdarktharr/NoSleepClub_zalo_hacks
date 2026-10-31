use serde::{Deserialize, Serialize};

use crate::{Error, Result, checked_elements, invalid};

/// Host-side admission limits. These do not bound native ORT workspace/weights;
/// the caller must enforce whole-process/OS budgets as well.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_text_bytes: usize,
    pub max_prompt_rows: usize,
    pub max_reference_frames: usize,
    pub max_new_frames: usize,
    pub max_chunk_frames: usize,
    pub max_heads_bytes: u64,
    pub max_pack_bytes: u64,
    pub max_state_bytes: usize,
    pub max_pcm_samples: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_text_bytes: 16_384,
            max_prompt_rows: 1024,
            max_reference_frames: 200,
            max_new_frames: 768,
            max_chunk_frames: 25,
            max_heads_bytes: 128 * 1024 * 1024,
            max_pack_bytes: 4_000_000_000,
            max_state_bytes: 256 * 1024 * 1024,
            max_pcm_samples: 48_000 * 60,
        }
    }
}

impl Limits {
    pub fn validate(&self) -> Result<()> {
        if self.max_text_bytes == 0
            || self.max_text_bytes > 1_048_576
            || self.max_prompt_rows == 0
            || self.max_prompt_rows > 8192
            || self.max_reference_frames > 1000
            || self.max_new_frames == 0
            || self.max_new_frames > 3000
            || self.max_chunk_frames == 0
            || self.max_chunk_frames > 100
            || self.max_heads_bytes == 0
            || self.max_heads_bytes > 512 * 1024 * 1024
            || self.max_pack_bytes == 0
            || self.max_pack_bytes > 15_000_000_000
            || self.max_state_bytes == 0
            || self.max_state_bytes > 1_000_000_000
            || self.max_pcm_samples == 0
            || self.max_pcm_samples > 48_000 * 180
        {
            return Err(invalid("invalid or excessive host limits"));
        }
        Ok(())
    }
}

/// Fields used by the v3 Turbo update architecture. Unrelated transformer
/// metadata is accepted, but all used dimensions/IDs are validated before use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub text_vocab_size: usize,
    pub audio_vocab_size: usize,
    pub n_vq: usize,
    pub hidden_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    #[serde(default)]
    pub head_dim: Option<usize>,
    #[serde(default = "default_positions")]
    pub max_position_embeddings: usize,
    #[serde(default = "one")]
    pub local_num_hidden_layers: usize,
    #[serde(default = "eight")]
    pub local_num_attention_heads: usize,
    pub audio_pad_token_id: i64,
    pub text_prompt_start_token_id: i64,
    pub text_prompt_end_token_id: i64,
    pub speech_generation_start_token_id: i64,
    pub speech_generation_end_token_id: i64,
    pub audio_ref_slot_token_id: i64,
    #[serde(default)]
    pub unk_token_id: Option<i64>,
    #[serde(default = "default_style")]
    pub default_style_token_id: i64,
    #[serde(default)]
    pub use_speaker_embedding: bool,
    #[serde(default = "speaker_dim")]
    pub speaker_embedding_dim: usize,
    #[serde(default = "sample_rate")]
    pub audio_sample_rate: usize,
}

fn one() -> usize {
    1
}
fn eight() -> usize {
    8
}
fn default_positions() -> usize {
    2048
}
fn default_style() -> i64 {
    16
}
fn speaker_dim() -> usize {
    192
}
fn sample_rate() -> usize {
    48_000
}

impl ModelConfig {
    pub fn validate(&self, limits: &Limits) -> Result<()> {
        limits.validate()?;
        if !(1..=64).contains(&self.n_vq)
            || !(1..=4096).contains(&self.hidden_size)
            || !(1..=65_536).contains(&self.text_vocab_size)
            || !(2..=65_536).contains(&self.audio_vocab_size)
            || !(1..=64).contains(&self.num_hidden_layers)
            || !(1..=8).contains(&self.local_num_hidden_layers)
            || self.num_attention_heads == 0
            || self.num_attention_heads > 64
            || self.num_key_value_heads == 0
            || self.num_key_value_heads > self.num_attention_heads
            || self.local_num_attention_heads == 0
            || self.local_num_attention_heads > 64
            || !self.hidden_size.is_multiple_of(self.num_attention_heads)
            || !self
                .hidden_size
                .is_multiple_of(self.local_num_attention_heads)
            || self
                .head_dim
                .is_some_and(|v| v != self.hidden_size / self.num_attention_heads)
            || self.max_position_embeddings == 0
            || self.max_position_embeddings > 16_384
            || self.audio_sample_rate != 48_000
            || self.speaker_embedding_dim == 0
            || self.speaker_embedding_dim > 4096
            || self.audio_pad_token_id < 0
        {
            return Err(invalid("unsupported v3 Turbo dimensions/sample rate"));
        }
        for id in [
            self.text_prompt_start_token_id,
            self.text_prompt_end_token_id,
            self.speech_generation_start_token_id,
            self.speech_generation_end_token_id,
            self.audio_ref_slot_token_id,
            self.default_style_token_id,
        ] {
            if id < 0 || id as usize >= self.text_vocab_size {
                return Err(invalid("special text token outside vocabulary"));
            }
        }
        if self
            .unk_token_id
            .is_some_and(|id| id < 0 || id as usize >= self.text_vocab_size)
        {
            return Err(invalid("unknown token ID outside vocabulary"));
        }
        let heads = checked_elements(&[
            self.hidden_size,
            self.text_vocab_size
                .checked_add(
                    self.n_vq
                        .checked_mul(self.audio_vocab_size)
                        .ok_or_else(|| Error::Limit("embedding dimensions".into()))?,
                )
                .ok_or_else(|| Error::Limit("embedding dimensions".into()))?,
        ])?;
        if heads
            .checked_mul(4)
            .is_none_or(|bytes| bytes as u64 > limits.max_heads_bytes)
        {
            return Err(Error::Limit("embedding/head memory budget".into()));
        }
        Ok(())
    }

    pub fn head_size(&self) -> usize {
        self.hidden_size / self.num_attention_heads
    }
    pub fn local_head_size(&self) -> usize {
        self.hidden_size / self.local_num_attention_heads
    }

    pub fn check_backbone_state(&self, positions: usize, limits: &Limits) -> Result<()> {
        if positions > self.max_position_embeddings {
            return Err(Error::Limit("model context length".into()));
        }
        let bytes = checked_elements(&[
            2,
            self.num_hidden_layers,
            self.num_key_value_heads,
            positions,
            self.head_size(),
            4,
        ])?;
        if bytes > limits.max_state_bytes {
            return Err(Error::Limit("KV cache budget".into()));
        }
        Ok(())
    }
}
