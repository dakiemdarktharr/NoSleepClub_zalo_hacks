// Modified Rust translation of VieNeu prompt/frame-cap behavior. See NOTICE.
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

use crate::{Error, Limits, ModelConfig, Result, checked_elements, invalid};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct VoicePreset {
    #[serde(default)]
    pub speaker_embedding: Option<Vec<f32>>,
    #[serde(default)]
    pub ref_codes: Option<Vec<Vec<i64>>>,
}

impl VoicePreset {
    pub fn validate(&self, cfg: &ModelConfig, limits: &Limits) -> Result<()> {
        if cfg.use_speaker_embedding {
            let embedding = self
                .speaker_embedding
                .as_ref()
                .ok_or_else(|| invalid("preset requires speaker embedding"))?;
            if embedding.len() != cfg.speaker_embedding_dim
                || embedding.iter().any(|v| !v.is_finite())
                || !embedding.iter().any(|v| *v != 0.0)
            {
                return Err(invalid("invalid speaker embedding"));
            }
        } else if self
            .speaker_embedding
            .as_ref()
            .is_some_and(|v| v.iter().any(|x| !x.is_finite()))
        {
            return Err(invalid("nonfinite optional speaker embedding"));
        }
        if let Some(codes) = &self.ref_codes {
            if codes.len() > limits.max_reference_frames {
                return Err(Error::Limit("reference frames".into()));
            }
            if codes.iter().any(|row| {
                row.len() != cfg.n_vq
                    || row
                        .iter()
                        .any(|v| *v < 0 || *v as usize >= cfg.audio_vocab_size)
            }) {
                return Err(invalid("reference code shape/value"));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Prompt {
    rows: Vec<i64>,
    width: usize,
}

impl Prompt {
    pub fn build(
        tokenizer: &Tokenizer,
        phonemes: &str,
        voice: &VoicePreset,
        cfg: &ModelConfig,
        limits: &Limits,
    ) -> Result<Self> {
        if phonemes.is_empty() || phonemes.len() > limits.max_text_bytes {
            return Err(Error::Limit("empty/oversized phoneme input".into()));
        }
        let encoded = tokenizer
            .encode(phonemes, false)
            .map_err(|e| invalid(format!("phoneme tokenization: {e}")))?;
        let ids: Vec<i64> = encoded.get_ids().iter().map(|id| i64::from(*id)).collect();
        if cfg
            .unk_token_id
            .is_some_and(|unknown| ids.contains(&unknown))
        {
            return Err(invalid("phonemes contain unknown tokenizer symbols"));
        }
        Self::from_token_ids(&ids, voice, cfg, limits)
    }

    pub fn from_token_ids(
        ids: &[i64],
        voice: &VoicePreset,
        cfg: &ModelConfig,
        limits: &Limits,
    ) -> Result<Self> {
        cfg.validate(limits)?;
        voice.validate(cfg, limits)?;
        if ids.is_empty()
            || ids
                .iter()
                .any(|id| *id < 0 || *id as usize >= cfg.text_vocab_size)
        {
            return Err(invalid("empty or out-of-vocabulary phoneme tokens"));
        }
        let refs = voice.ref_codes.as_ref().map_or(0, Vec::len);
        let count = ids
            .len()
            .checked_add(3)
            .and_then(|n| n.checked_add(refs))
            .ok_or_else(|| Error::Limit("prompt size overflow".into()))?;
        if count > limits.max_prompt_rows || count > cfg.max_position_embeddings {
            return Err(Error::Limit("prompt rows/context".into()));
        }
        let width = cfg.n_vq + 1;
        let mut rows = vec![cfg.audio_pad_token_id; checked_elements(&[count, width])?];
        let text_ids = std::iter::once(cfg.default_style_token_id)
            .chain(std::iter::once(cfg.text_prompt_start_token_id))
            .chain(ids.iter().copied())
            .chain(std::iter::once(cfg.text_prompt_end_token_id));
        for (index, id) in text_ids.enumerate() {
            rows[index * width] = id;
        }
        if let Some(codes) = &voice.ref_codes {
            for (i, frame) in codes.iter().enumerate() {
                let start = (ids.len() + 3 + i) * width;
                rows[start] = cfg.audio_ref_slot_token_id;
                rows[start + 1..start + width].copy_from_slice(frame);
            }
        }
        Ok(Self { rows, width })
    }

    pub fn rows(&self) -> &[i64] {
        &self.rows
    }
    pub fn row_count(&self) -> usize {
        self.rows.len() / self.width
    }
    pub fn width(&self) -> usize {
        self.width
    }
}

fn strip_markup(phonemes: &str) -> std::borrow::Cow<'_, str> {
    static MARKUP: OnceLock<Regex> = OnceLock::new();
    MARKUP
        .get_or_init(|| Regex::new(r"<\|emotion_\d+\|>|</?en>").expect("constant regex"))
        .replace_all(phonemes, "")
}

pub fn syllable_count(phonemes: &str) -> usize {
    const VOWELS: &str = "aeiouyæɐɑɒɔəɘɛɜɤɯɵøœʉʊʌɪɨɚɝᵻᵿ";
    let stripped = strip_markup(phonemes);
    let mut total = 0;
    for token in stripped.split_whitespace() {
        let (mut groups, mut in_v, mut consonant_seen) = (0usize, false, true);
        for ch in token.chars() {
            if VOWELS.contains(ch) {
                if !in_v && consonant_seen {
                    groups += 1;
                }
                in_v = true;
                consonant_seen = false;
            } else if "ːˈˌ".contains(ch) || ch.is_numeric() {
                if "ˈˌ".contains(ch) && groups > 0 {
                    in_v = false;
                    consonant_seen = true;
                } else {
                    in_v = false;
                }
            } else {
                in_v = false;
                consonant_seen = true;
            }
        }
        if token.chars().any(char::is_alphabetic) {
            total += groups.max(1);
        }
    }
    total
}

pub fn max_expected_frames(phonemes: &str) -> usize {
    let stripped = strip_markup(phonemes);
    let len = stripped.chars().count();
    let mut cap = 24usize.saturating_add(len.saturating_mul(2));
    let emotion = phonemes.contains("<|emotion_");
    if emotion && !stripped.chars().any(char::is_alphabetic) {
        return cap.min(13);
    }
    if !emotion {
        let syllables = syllable_count(phonemes).max(1);
        if syllables <= 4 && len <= 24 * syllables {
            cap = cap.min(13 + 5 * (syllables - 1));
        }
    }
    cap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_syllables_and_emotion_cap_match_reference() {
        assert_eq!(syllable_count("kɹiːˈeɪt"), 2);
        assert_eq!(syllable_count("ɹˈeɪdɪˌoʊ"), 3);
        assert_eq!(syllable_count("kwˈaːɜ"), 1);
        assert_eq!(max_expected_frames("<|emotion_1|>"), 13);
        assert_eq!(max_expected_frames("bˈaːɜw"), 13);
        assert_eq!(max_expected_frames("bˈaːɜw kwˈaːɜ"), 18);
    }
}
