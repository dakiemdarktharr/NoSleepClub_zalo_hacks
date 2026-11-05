//! Explicit, hash-pinned local assets shared by audio and text entry points.
use crate::Result;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// Verify bytes before any model process is started. The GGUF header and hash
/// bind an asset; base/adapter compatibility still needs native validation.
pub fn pinned_lora(path: &Path, expected: &str) -> Result<PathBuf> {
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("LoRA requires a 64 hexadecimal SHA256 pin".into());
    }
    let path = path.canonicalize()?;
    let mut file = File::open(&path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || !(8..=512 * 1024 * 1024).contains(&metadata.len()) {
        return Err("LoRA adapter size exceeds the asset bound".into());
    }
    let mut header = [0u8; 8];
    file.read_exact(&mut header)?;
    if &header[..4] != b"GGUF" || u32::from_le_bytes(header[4..].try_into()?) != 3 {
        return Err("LoRA expects a native GGUF v3 adapter".into());
    }
    let mut hash = Sha256::new();
    hash.update(header);
    let mut buffer = [0u8; 65536];
    let mut bytes = 8u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        if bytes > metadata.len() {
            return Err("LoRA changed while hashing".into());
        }
        hash.update(&buffer[..count]);
    }
    if bytes != metadata.len() || format!("{:x}", hash.finalize()) != expected.to_ascii_lowercase()
    {
        return Err("LoRA asset hash differs".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_binds_exact_bytes_and_requires_native_header() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("adapter with spaces.gguf");
        // Header-only fixtures do not prove native compatibility or quality.
        let bytes = b"GGUF\x03\x00\x00\x00fixture";
        std::fs::write(&path, bytes).unwrap();
        let pin = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(
            pinned_lora(&path, &pin).unwrap(),
            path.canonicalize().unwrap()
        );
        assert!(pinned_lora(&path, &pin.to_ascii_uppercase()).is_ok());
        assert!(pinned_lora(&path, &"0".repeat(64)).is_err());
        for invalid in [b"GGUF\x02\x00\x00\x00fixture".as_slice(), b"not GGUF", b""] {
            std::fs::write(&path, invalid).unwrap();
            let pin = format!("{:x}", Sha256::digest(invalid));
            assert!(pinned_lora(&path, &pin).is_err());
        }
        assert!(pinned_lora(directory.path(), &pin).is_err());
        assert!(pinned_lora(&path, "invalid").is_err());
    }
}
