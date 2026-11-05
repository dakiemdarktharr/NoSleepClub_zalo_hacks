//! Isolated encoder probe: no LifeMate SDK, labels, LLM or evaluation claims.
use nora_lifemate::{
    Result,
    embedding::{E5Encoder, EmbeddingAssets},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Instant,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    scope: String,
    texts: Vec<String>,
}
fn main() -> Result<()> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 5 {
        return Err("expected ASSETS ASSETS_SHA256 FIXTURE FIXTURE_SHA256 NEW_OUTPUT".into());
    }
    let mut bytes = Vec::new();
    File::open(&a[2])?.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 || format!("{:x}", Sha256::digest(&bytes)) != a[3] {
        return Err("embedding fixture changed".into());
    }
    let fixture: Fixture = serde_json::from_slice(&bytes)?;
    if fixture.scope != "synthetic-e5-reference-v1"
        || fixture.texts.is_empty()
        || fixture.texts.len() > 16
    {
        return Err("invalid synthetic fixture".into());
    }
    let started = Instant::now();
    let mut model = E5Encoder::load(EmbeddingAssets::read(Path::new(&a[0]), &a[1])?)?;
    let mut output = Vec::new();
    for text in &fixture.texts {
        let begin = Instant::now();
        let embedding = model
            .encode(text)?
            .ok_or("synthetic fixture exceeds512tokens")?;
        output.push(serde_json::json!({"text_sha256":format!("{:x}",Sha256::digest(text.as_bytes())),"embedding":embedding,"elapsed_ms":begin.elapsed().as_secs_f64()*1000.}));
    }
    let result = serde_json::json!({"scope":"synthetic CPU E5 Rust host probe; not task accuracy","model_sha256":model.model_sha256,"tokenizer_sha256":model.tokenizer_sha256,"rows":output,"elapsed_ms":started.elapsed().as_secs_f64()*1000.});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&a[4])?;
    serde_json::to_writer(&mut file, &result)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    eprintln!(
        "E5 probe completed {} synthetic inputs; not LifeMate score",
        fixture.texts.len()
    );
    Ok(())
}
