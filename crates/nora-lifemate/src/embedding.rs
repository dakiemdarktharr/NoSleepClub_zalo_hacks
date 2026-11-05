//! Optional, offline CPU E5 encoder. No retrieval policy is promoted by this
//! host. Asset hashes precede native loading; long inputs are explicitly refused
//! rather than silently truncated. The actual task prompt is never shortened.
use crate::Result;
use ort::{
    environment::Environment,
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};
use tokenizers::{Encoding, Tokenizer};

pub const DIMENSIONS: usize = 384;
pub const MAX_TOKENS: usize = 512;

fn backend(error: impl std::fmt::Display) -> Box<dyn std::error::Error + Send + Sync> {
    format!("embedding backend: {error}").into()
}

fn load_tokenizer(path: &Path) -> Result<Tokenizer> {
    let mut tokenizer = Tokenizer::from_file(path)?;
    tokenizer.with_truncation(None)?;
    tokenizer.with_padding(None);
    Ok(tokenizer)
}

fn encode_tokens(tokenizer: &Tokenizer, text: &str) -> Result<Option<Encoding>> {
    if text.trim().is_empty() || text.len() > 65_536 {
        return Err("embedding input empty or exceeds text bound".into());
    }
    let encoded = tokenizer.encode(format!("query: {text}"), true)?;
    if encoded.len() > MAX_TOKENS {
        return Ok(None);
    }
    if encoded.is_empty() {
        return Err("embedding tokenizer returned no tokens".into());
    }
    Ok(Some(encoded))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedFile {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}
impl PinnedFile {
    pub fn verify(&self, maximum: u64) -> Result<()> {
        if self.bytes == 0
            || self.bytes > maximum
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid embedding asset pin or size".into());
        }
        let file = File::open(&self.path)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() != self.bytes {
            return Err("embedding asset length differs".into());
        }
        let mut input = BufReader::new(file).take(self.bytes + 1);
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 65_536];
        let mut count = 0;
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
            count += n as u64;
        }
        if count != self.bytes || format!("{:x}", digest.finalize()) != self.sha256 {
            return Err("embedding asset bytes changed".into());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingAssets {
    pub scope: String,
    pub runtime: PinnedFile,
    pub model: PinnedFile,
    pub tokenizer: PinnedFile,
    pub threads: usize,
}
impl EmbeddingAssets {
    pub fn read(path: &Path, sha256: &str) -> Result<Self> {
        let mut bytes = Vec::new();
        File::open(path)?.take(16_385).read_to_end(&mut bytes)?;
        if bytes.len() > 16_384 || format!("{:x}", Sha256::digest(&bytes)) != sha256 {
            return Err("embedding manifest changed or exceeds bound".into());
        }
        let assets: Self = serde_json::from_slice(&bytes)?;
        if assets.scope != "e5-small-cpu-experimental-v1" || !(1..=4).contains(&assets.threads) {
            return Err("embedding manifest scope/thread count invalid".into());
        }
        Ok(assets)
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EncodedText {
    pub tokens: usize,
    pub vector: Vec<f32>,
}

pub struct E5Encoder {
    session: Session,
    tokenizer: Tokenizer,
    _environment: Environment,
    pub model_sha256: String,
    pub tokenizer_sha256: String,
}
impl E5Encoder {
    pub fn load(assets: EmbeddingAssets) -> Result<Self> {
        if assets.scope != "e5-small-cpu-experimental-v1" || !(1..=4).contains(&assets.threads) {
            return Err("embedding manifest scope/thread count invalid".into());
        }
        // All three checks happen before executing the native runtime.
        assets.runtime.verify(100_000_000)?;
        assets.model.verify(550_000_000)?;
        assets.tokenizer.verify(32_000_000)?;
        tokenizers::utils::parallelism::set_parallelism(false);
        let tokenizer = load_tokenizer(&assets.tokenizer.path)?;
        let environment = ort::init_from(&assets.runtime.path)?
            .with_telemetry(false)
            .build()?;
        let session = Session::builder(&environment)?
            .with_intra_threads(assets.threads)
            .map_err(backend)?
            .with_inter_threads(1)
            .map_err(backend)?
            .with_parallel_execution(false)
            .map_err(backend)?
            .with_intra_op_spinning(false)
            .map_err(backend)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(backend)?
            .commit_from_file(&assets.model.path)?;
        Ok(Self {
            session,
            tokenizer,
            _environment: environment,
            model_sha256: assets.model.sha256,
            tokenizer_sha256: assets.tokenizer.sha256,
        })
    }

    /// E5 symmetric sentence similarity uses `query:` on both sides. `None`
    /// means the complete tokenization exceeded the model's 512-token limit.
    pub fn encode(&mut self, text: &str) -> Result<Option<EncodedText>> {
        let Some(encoded) = encode_tokens(&self.tokenizer, text)? else {
            return Ok(None);
        };
        let length = encoded.len();
        let ids = encoded
            .get_ids()
            .iter()
            .map(|x| i64::from(*x))
            .collect::<Vec<_>>();
        let mask = encoded
            .get_attention_mask()
            .iter()
            .map(|x| i64::from(*x))
            .collect::<Vec<_>>();
        let types = encoded
            .get_type_ids()
            .iter()
            .map(|x| i64::from(*x))
            .collect::<Vec<_>>();
        let output = self.session.run(ort::inputs![
            "input_ids"=>TensorRef::from_array_view(([1usize,length],ids.as_slice()))?,
            "attention_mask"=>TensorRef::from_array_view(([1usize,length],mask.as_slice()))?,
            "token_type_ids"=>TensorRef::from_array_view(([1usize,length],types.as_slice()))?,
        ])?;
        let (shape, hidden) = output
            .get("last_hidden_state")
            .ok_or("embedding graph has no hidden state")?
            .try_extract_tensor::<f32>()?;
        if shape.as_ref() != [1, length as i64, DIMENSIONS as i64] {
            return Err("embedding graph output shape differs".into());
        }
        Ok(Some(EncodedText {
            tokens: length,
            vector: mean_normalize(hidden, &mask, DIMENSIONS)?,
        }))
    }
}

fn mean_normalize(hidden: &[f32], mask: &[i64], dimensions: usize) -> Result<Vec<f32>> {
    if dimensions == 0
        || mask.is_empty()
        || mask.len().checked_mul(dimensions) != Some(hidden.len())
        || mask.iter().any(|m| !matches!(m, 0 | 1))
        || hidden.iter().any(|x| !x.is_finite())
    {
        return Err("invalid embedding tensor/mask".into());
    }
    let count = mask.iter().filter(|&&m| m == 1).count();
    if count == 0 {
        return Err("embedding has no attended token".into());
    }
    let mut pooled = vec![0.0f64; dimensions];
    for (row, &m) in hidden.chunks_exact(dimensions).zip(mask) {
        if m == 1 {
            for (sum, &value) in pooled.iter_mut().zip(row) {
                *sum += f64::from(value);
            }
        }
    }
    for value in &mut pooled {
        *value /= count as f64;
    }
    let norm = pooled.iter().map(|x| x * x).sum::<f64>().sqrt();
    if !norm.is_finite() || norm <= 1e-12 {
        return Err("embedding norm is invalid".into());
    }
    Ok(pooled.iter().map(|x| (x / norm) as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_tokenizer_truncation_cannot_hide_an_oversized_query() {
        use tokenizers::{
            TruncationParams, models::wordlevel::WordLevel, pre_tokenizers::whitespace::Whitespace,
        };
        let vocabulary = [("[UNK]".to_owned(), 0), ("word".to_owned(), 1)]
            .into_iter()
            .collect();
        let model = WordLevel::builder()
            .vocab(vocabulary)
            .unk_token("[UNK]".into())
            .build()
            .unwrap();
        let mut tokenizer = Tokenizer::new(model);
        tokenizer.with_pre_tokenizer(Some(Whitespace));
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: 8,
                ..Default::default()
            }))
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tokenizer.json");
        tokenizer.save(&path, false).unwrap();
        let loaded = load_tokenizer(&path).unwrap();
        assert!(
            encode_tokens(&loaded, &"word ".repeat(512))
                .unwrap()
                .is_none()
        );
        assert!(encode_tokens(&loaded, "word").unwrap().is_some());
        assert!(encode_tokens(&loaded, "").is_err());
    }
    #[test]
    fn pooling_ignores_padding_and_is_invariant_to_repeating_attended_tokens() {
        let a = mean_normalize(&[3., 4., 999., -999.], &[1, 0], 2).unwrap();
        let b = mean_normalize(&[3., 4., 3., 4.], &[1, 1], 2).unwrap();
        assert_eq!(a, vec![0.6, 0.8]);
        assert_eq!(a, b);
    }
    #[test]
    fn malformed_or_nonfinite_embeddings_are_not_indexed() {
        for (hidden, mask, dim) in [
            (vec![1., 2.], vec![0], 2),
            (vec![1.], vec![1], 2),
            (vec![0., 0.], vec![1], 2),
            (vec![f32::NAN, 2.], vec![1], 2),
            (vec![1., 2.], vec![2], 2),
            (vec![], vec![], 0),
        ] {
            assert!(mean_normalize(&hidden, &mask, dim).is_err());
        }
    }
    #[test]
    fn changed_asset_is_rejected_before_native_loading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("asset");
        std::fs::write(&path, b"known").unwrap();
        let pin = PinnedFile {
            path: path.clone(),
            bytes: 5,
            sha256: format!("{:x}", Sha256::digest(b"known")),
        };
        pin.verify(5).unwrap();
        assert!(pin.verify(4).is_err());
        std::fs::write(&path, b"other").unwrap();
        assert!(pin.verify(5).is_err());
        std::fs::write(&path, b"longer").unwrap();
        assert!(pin.verify(6).is_err());
    }
}
