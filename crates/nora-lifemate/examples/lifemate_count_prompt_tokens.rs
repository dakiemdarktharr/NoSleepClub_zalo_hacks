//! Offline raw-text token counts. No model inference, SDK or quality claims.
use nora_lifemate::Result;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read};
use tokenizers::Tokenizer;

fn read_bounded(path: &str, bound: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(bound + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > bound {
        return Err("token-count input exceeds bound".into());
    }
    Ok(bytes)
}

fn main() -> Result<()> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if !(3..=18).contains(&a.len()) {
        return Err("expected TOKENIZER SHA256 TEXT_FILE [TEXT_FILE ...]".into());
    }
    let bytes = read_bounded(&a[0], 32_000_000)?;
    if format!("{:x}", Sha256::digest(&bytes)) != a[1] {
        return Err("tokenizer hash changed".into());
    }
    let mut tokenizer = Tokenizer::from_bytes(&bytes)?;
    tokenizer.with_truncation(None)?;
    tokenizer.with_padding(None);
    for path in &a[2..] {
        let text = String::from_utf8(read_bounded(path, 1_000_000)?)?;
        let encoded = tokenizer.encode(text.as_str(), false)?;
        println!(
            "{}",
            serde_json::json!({
                "scope":"raw text only; excludes chat-template overhead; no inference",
                "file":path,"text_sha256":format!("{:x}",Sha256::digest(text.as_bytes())),
                "tokenizer_sha256":a[1],"bytes":text.len(),"tokens":encoded.len()
            })
        );
    }
    Ok(())
}
