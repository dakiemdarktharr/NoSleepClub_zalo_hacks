//! Optional train-only semantic index. Query inference happens inside the task
//! clock. Neither the index nor its encoder can call the SDK or read task state.
use crate::{
    Result,
    embedding::{DIMENSIONS, E5Encoder, EncodedText, MAX_TOKENS},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRecord {
    pub sample_id: String,
    pub request_sha256: String,
    /// None explicitly records a complete input exceeding the encoder limit.
    pub embedding: Option<EncodedText>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticCorpus {
    pub scope: String,
    pub corpus_sha256: String,
    pub model_sha256: String,
    pub tokenizer_sha256: String,
    pub prefix: String,
    pub rows: Vec<SemanticRecord>,
}

pub struct SemanticIndex {
    vectors: Vec<Option<Vec<f32>>>,
    pub sha256: String,
    model_sha256: String,
    tokenizer_sha256: String,
}

fn valid_vector(encoded: &EncodedText) -> bool {
    let norm = encoded
        .vector
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>();
    (1..=MAX_TOKENS).contains(&encoded.tokens)
        && encoded.vector.len() == DIMENSIONS
        && encoded.vector.iter().all(|v| v.is_finite())
        && (norm - 1.).abs() <= 1e-4
}

impl SemanticIndex {
    pub fn read(
        path: &Path,
        expected_sha: &str,
        corpus_sha: &str,
        requests: &[(&str, &str)],
    ) -> Result<Self> {
        let mut bytes = Vec::new();
        File::open(path)?.take(32_000_001).read_to_end(&mut bytes)?;
        if bytes.len() > 32_000_000 || format!("{:x}", Sha256::digest(&bytes)) != expected_sha {
            return Err("semantic index changed or exceeds bound".into());
        }
        let corpus: SemanticCorpus = serde_json::from_slice(&bytes)?;
        if corpus.scope != "internal-train-e5-embeddings-v1"
            || corpus.corpus_sha256 != corpus_sha
            || corpus.prefix != "query: "
            || corpus.rows.len() != requests.len()
            || requests.is_empty()
        {
            return Err("semantic index scope, coverage or corpus mismatch".into());
        }
        let mut by_id = BTreeMap::new();
        for row in corpus.rows {
            if row.embedding.as_ref().is_some_and(|v| !valid_vector(v))
                || by_id.insert(row.sample_id.clone(), row).is_some()
            {
                return Err("semantic index has duplicate ID or invalid embedding".into());
            }
        }
        let mut vectors = Vec::with_capacity(requests.len());
        for &(id, text) in requests {
            let row = by_id.remove(id).ok_or("semantic index missing train ID")?;
            if row.request_sha256 != format!("{:x}", Sha256::digest(text.as_bytes())) {
                return Err("semantic index request differs from training corpus".into());
            }
            vectors.push(row.embedding.map(|v| v.vector));
        }
        if !by_id.is_empty() {
            return Err("semantic index contains non-training rows".into());
        }
        Ok(Self {
            vectors,
            sha256: expected_sha.into(),
            model_sha256: corpus.model_sha256,
            tokenizer_sha256: corpus.tokenizer_sha256,
        })
    }
}

pub struct SemanticRetriever {
    index: SemanticIndex,
    encoder: E5Encoder,
    cached: Option<(String, Option<Vec<Option<f64>>>)>,
}

impl SemanticRetriever {
    pub fn new(index: SemanticIndex, encoder: E5Encoder) -> Result<Self> {
        if index.model_sha256 != encoder.model_sha256
            || index.tokenizer_sha256 != encoder.tokenizer_sha256
        {
            return Err("semantic index and query encoder assets differ".into());
        }
        Ok(Self {
            index,
            encoder,
            cached: None,
        })
    }

    pub fn index_sha256(&self) -> &str {
        &self.index.sha256
    }

    /// Cache only the last exact live request. There is no precomputed query
    /// cache or validation input in the train index. Long queries use BM25.
    pub fn scores(&mut self, text: &str) -> Result<Option<Vec<Option<f64>>>> {
        if let Some((prior, scores)) = &self.cached
            && prior == text
        {
            return Ok(scores.clone());
        }
        let scores = self.encoder.encode(text)?.map(|encoded| {
            self.index
                .vectors
                .iter()
                .map(|vector| {
                    vector.as_ref().map(|v| {
                        v.iter()
                            .zip(&encoded.vector)
                            .map(|(a, b)| f64::from(*a) * f64::from(*b))
                            .sum()
                    })
                })
                .collect::<Vec<_>>()
        });
        self.cached = Some((text.into(), scores.clone()));
        Ok(scores)
    }
}

/// Equal-weight reciprocal rank fusion, k=60, with stable ID tie breaking.
/// Only caller-supplied eligible rows are ranked. Zero lexical overlap supplies
/// no lexical vote; a missing semantic vector supplies no semantic vote.
pub(crate) fn fused_ranking(candidates: &[(usize, &str, f64, Option<f64>)]) -> Vec<(usize, f64)> {
    let mut lexical = candidates.iter().filter(|r| r.2 > 0.).collect::<Vec<_>>();
    lexical.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.1.cmp(b.1)));
    let mut semantic = candidates
        .iter()
        .filter_map(|r| r.3.map(|score| (r, score)))
        .collect::<Vec<_>>();
    semantic.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.1.cmp(b.0.1)));
    let mut scores = BTreeMap::<usize, (f64, &str)>::new();
    for (rank, row) in lexical.iter().enumerate() {
        scores.entry(row.0).or_insert((0., row.1)).0 += 1. / (61. + rank as f64);
    }
    for (rank, (row, _)) in semantic.iter().enumerate() {
        scores.entry(row.0).or_insert((0., row.1)).0 += 1. / (61. + rank as f64);
    }
    let mut ranked = scores.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then(a.1.1.cmp(b.1.1)));
    ranked.into_iter().map(|(i, (s, _))| (i, s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn corpus() -> SemanticCorpus {
        let mut vector = vec![0.; DIMENSIONS];
        vector[0] = 1.;
        SemanticCorpus {
            scope: "internal-train-e5-embeddings-v1".into(),
            corpus_sha256: "corpus".into(),
            model_sha256: "model".into(),
            tokenizer_sha256: "tokenizer".into(),
            prefix: "query: ".into(),
            rows: vec![SemanticRecord {
                sample_id: "train".into(),
                request_sha256: format!("{:x}", Sha256::digest(b"original")),
                embedding: Some(EncodedText { tokens: 8, vector }),
            }],
        }
    }

    #[test]
    fn pins_training_identity_and_original_request_are_mandatory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");
        let bytes = serde_json::to_vec(&corpus()).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let sha = format!("{:x}", Sha256::digest(bytes));
        let good = [("train", "original")];
        assert!(SemanticIndex::read(&path, &sha, "corpus", &good).is_ok());
        for (hash, source, requests) in [
            ("changed", "corpus", good),
            (&sha, "changed", good),
            (&sha, "corpus", [("validation", "original")]),
            (&sha, "corpus", [("train", "changed")]),
        ] {
            assert!(SemanticIndex::read(&path, hash, source, &requests).is_err());
        }
    }

    #[test]
    fn unnormalized_vectors_are_rejected_and_missing_vectors_are_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");
        let mut data = corpus();
        data.rows[0].embedding.as_mut().unwrap().vector[0] = 2.;
        for accepted in [false, true] {
            if accepted {
                data.rows[0].embedding = None;
            }
            let bytes = serde_json::to_vec(&data).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            let sha = format!("{:x}", Sha256::digest(bytes));
            assert_eq!(
                SemanticIndex::read(&path, &sha, "corpus", &[("train", "original")]).is_ok(),
                accepted
            );
        }
    }

    #[test]
    fn complementary_rankings_fuse_with_stable_ties_and_no_phantom_votes() {
        let rows = [
            (0, "b", 9., Some(0.8)),
            (1, "a", 1., Some(0.9)),
            (2, "c", 0., None),
        ];
        let ranked = fused_ranking(&rows);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].0, 1);
        assert_eq!(ranked[0].1, ranked[1].1);
        assert_eq!(fused_ranking(&[(3, "d", 0., Some(0.1))])[0].0, 3);
        assert!(fused_ranking(&[(3, "d", 0., None)]).is_empty());
    }
}
