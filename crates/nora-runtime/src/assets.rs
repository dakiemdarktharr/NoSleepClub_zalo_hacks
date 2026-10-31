use crate::Result;
use cap_std::{ambient_authority, fs::Dir};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    id: String,
    files: Vec<Entry>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetReport {
    pub verified_files: usize,
    pub bytes: u64,
    pub publisher_authenticated: bool,
}

pub fn verify_manifest(root: &Path, encoded: &str) -> Result<AssetReport> {
    if encoded.len() > 1_048_576 {
        return Err("asset catalog exceeds limit".into());
    }
    let manifest: Manifest = serde_json::from_str(encoded)?;
    if manifest.schema_version != 1
        || manifest.id.is_empty()
        || manifest.files.is_empty()
        || manifest.files.len() > 64
    {
        return Err("invalid asset catalog".into());
    }
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    let mut report = AssetReport {
        verified_files: 0,
        bytes: 0,
        publisher_authenticated: false,
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut buffer = [0u8; 65_536];
    for entry in manifest.files {
        let path = Path::new(&entry.path);
        if entry.path.is_empty()
            || entry.path.contains(['\\', ':', '\0'])
            || !path.components().all(|c| matches!(c, Component::Normal(_)))
            || !seen.insert(entry.path.to_ascii_lowercase())
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || entry.bytes == 0
        {
            return Err("unsafe asset catalog entry".into());
        }
        report.bytes = report
            .bytes
            .checked_add(entry.bytes)
            .ok_or("asset size overflow")?;
        if report.bytes > nora_platform::DISK_LIMIT_BYTES {
            return Err("asset budget exceeded".into());
        }
        let mut file = directory.open(path)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() != entry.bytes {
            return Err("asset is missing or has changed size".into());
        }
        let mut digest = Sha256::new();
        let mut read = 0u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            read = read
                .checked_add(count as u64)
                .ok_or("asset size overflow")?;
            if read > entry.bytes {
                return Err("asset grew during integrity validation".into());
            }
            digest.update(&buffer[..count]);
        }
        if read != entry.bytes
            || !format!("{:x}", digest.finalize()).eq_ignore_ascii_case(&entry.sha256)
        {
            return Err("asset integrity mismatch; refusing native inference".into());
        }
        report.verified_files += 1;
    }
    Ok(report)
}

#[derive(Debug, Clone)]
pub struct VoiceAssets {
    pub root: PathBuf,
}
impl VoiceAssets {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn ort(&self) -> PathBuf {
        self.root.join("ort/onnxruntime.dll")
    }
    pub fn vad(&self) -> PathBuf {
        self.root.join("silero/silero_vad.onnx")
    }
    pub fn asr(&self) -> PathBuf {
        self.root.join("whisper/ggml-large-v3-turbo-q5_0.bin")
    }
    pub fn tts(&self) -> PathBuf {
        self.root.join("vieneu")
    }
    pub fn codec(&self) -> PathBuf {
        self.tts().join("codec")
    }
    pub fn g2p(&self) -> PathBuf {
        self.root.join("g2p/sea_g2p.bin")
    }
    pub fn verify_tts(&self) -> Result<Vec<AssetReport>> {
        Ok(vec![
            verify_manifest(
                &self.tts(),
                include_str!("../../../assets/manifests/vieneu-v3-turbo-fp32.json"),
            )?,
            verify_manifest(
                &self.root.join("g2p"),
                include_str!("../../../assets/manifests/sea-g2p.json"),
            )?,
            verify_manifest(
                &self.root.join("ort"),
                include_str!("../../../assets/manifests/onnxruntime-win-x64-cpu.json"),
            )?,
        ])
    }
    pub fn verify_asr(&self) -> Result<AssetReport> {
        verify_manifest(
            &self.root.join("whisper"),
            include_str!("../../../assets/manifests/whisper-large-v3-turbo-q5_0.json"),
        )
    }
    pub fn verify_vad(&self) -> Result<AssetReport> {
        verify_manifest(
            &self.root.join("silero"),
            include_str!("../../../assets/manifests/silero-vad.json"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traversal_and_corrupt_assets_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let invalid = r#"{"schema_version":1,"id":"x","files":[{"path":"../outside","bytes":1,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#;
        assert!(verify_manifest(root.path(), invalid).is_err());
        std::fs::write(root.path().join("test"), b"bad").unwrap();
        let invalid = invalid
            .replace("../outside", "test")
            .replace("\"bytes\":1", "\"bytes\":3");
        assert!(verify_manifest(root.path(), &invalid).is_err());
    }
}
