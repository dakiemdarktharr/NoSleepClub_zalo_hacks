//! Build embeddings only for the exact pinned internal training corpus.
use nora_lifemate::{
    Result,
    cohort::EvaluationCohort,
    embedding::{E5Encoder, EmbeddingAssets},
    retrieval::ExampleIndex,
    semantic_retrieval::{SemanticCorpus, SemanticRecord},
};
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::Path, time::Instant};

fn main() -> Result<()> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 7 {
        return Err("expected CORPUS SHA256 TRAIN_IDS SHA256 ASSETS SHA256 NEW_OUTPUT".into());
    }
    let train = EvaluationCohort::from_file(Path::new(&a[2]), &a[3])?;
    let corpus = ExampleIndex::from_file(Path::new(&a[0]), &a[1], &train)?;
    if Path::new(&a[6]).exists() {
        return Err("refuse to overwrite embedding index".into());
    }
    let started = Instant::now();
    let mut encoder = E5Encoder::load(EmbeddingAssets::read(Path::new(&a[4]), &a[5])?)?;
    let mut rows = Vec::with_capacity(train.len());
    for (id, text) in corpus.training_requests() {
        rows.push(SemanticRecord {
            sample_id: id.into(),
            request_sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            embedding: encoder.encode(text)?,
        });
    }
    let skipped = rows.iter().filter(|r| r.embedding.is_none()).count();
    let result = SemanticCorpus {
        scope: "internal-train-e5-embeddings-v1".into(),
        corpus_sha256: a[1].clone(),
        model_sha256: encoder.model_sha256,
        tokenizer_sha256: encoder.tokenizer_sha256,
        prefix: "query: ".into(),
        rows,
    };
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&a[6])?;
    serde_json::to_writer(&mut out, &result)?;
    out.write_all(b"\n")?;
    out.sync_all()?;
    eprintln!(
        "{} training rows, {skipped} explicitly over limit, {:.3}s; not task accuracy",
        train.len(),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
