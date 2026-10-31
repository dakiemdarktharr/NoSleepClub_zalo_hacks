//! Model data validation. Trust in the manifest/publisher must be established
//! separately; a matching hash does not prove a model is safe or accurate.

use std::{
    collections::BTreeSet,
    fs,
    io::{BufReader, Read},
    path::{Component, Path},
};

use cap_std::{ambient_authority, fs::Dir};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::commands::Result;

pub const MAX_MANIFEST_BYTES: u64 = 1_048_576;
const MAX_FILES: usize = 64;
const MAX_PACK_BYTES: u64 = 15_000_000_000;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub files: Vec<Asset>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub pack_id: String,
    pub files_verified: usize,
    pub total_bytes: u64,
    pub integrity_verified: bool,
    pub publisher_authenticated: bool,
}

/// Portable relative paths only; reject Windows prefixes/ADS and traversal even
/// when validation is executed on another OS. Dir additionally enforces opening
/// beneath a directory capability, including during path races.
pub fn relative_path(value: &str) -> Result<&Path> {
    if value.is_empty() || value.len() > 1024 || value.contains(['\\', ':', '\0']) {
        return Err("path must be a bounded portable relative path".into());
    }
    if value
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.ends_with([' ', '.']))
    {
        return Err("path contains an unsafe component".into());
    }
    let path = Path::new(value);
    if !path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("absolute paths and parent traversal are not permitted".into());
    }
    for part in value.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err("reserved device path is not permitted".into());
        }
    }
    Ok(path)
}

pub fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>> {
    let sentinel = max.checked_add(1).ok_or("invalid input limit")?;
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > max {
        return Err("input is not a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(sentinel).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err("input grew beyond limit".into());
    }
    Ok(bytes)
}

pub fn digest_reader(mut reader: impl Read, max: u64) -> Result<(String, u64)> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65_536];
    let mut total = 0u64;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or("asset length overflow")?;
        if total > max {
            return Err("asset grew beyond declared size".into());
        }
        digest.update(&buffer[..count]);
    }
    Ok((format!("{:x}", digest.finalize()), total))
}

pub fn verify(root: &Path, manifest_path: &Path) -> Result<Report> {
    let encoded = read_bounded(manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest = serde_json::from_slice(&encoded)?;
    if manifest.schema_version != 1
        || manifest.id.trim().is_empty()
        || manifest.id.len() > 256
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_FILES
    {
        return Err("unsupported or unbounded asset manifest".into());
    }
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    let mut seen = BTreeSet::new();
    let mut declared = 0u64;
    for entry in &manifest.files {
        relative_path(&entry.path)?;
        if !seen.insert(entry.path.to_ascii_lowercase()) {
            return Err("duplicate/case-colliding asset path".into());
        }
        if entry.bytes == 0
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid declared size or SHA256".into());
        }
        declared = declared
            .checked_add(entry.bytes)
            .ok_or("pack size overflow")?;
        if declared > MAX_PACK_BYTES {
            return Err("pack exceeds client disk budget".into());
        }
    }
    let mut total = 0u64;
    for entry in &manifest.files {
        let file = directory.open(relative_path(&entry.path)?)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() != entry.bytes {
            return Err(format!("asset size mismatch: {}", entry.path).into());
        }
        let (actual, size) = digest_reader(BufReader::new(file), entry.bytes)?;
        if size != entry.bytes || !actual.eq_ignore_ascii_case(&entry.sha256) {
            return Err(format!("asset digest mismatch: {}", entry.path).into());
        }
        total = total.checked_add(size).ok_or("verified length overflow")?;
    }
    Ok(Report {
        pack_id: manifest.id,
        files_verified: manifest.files.len(),
        total_bytes: total,
        integrity_verified: true,
        publisher_authenticated: false,
    })
}

/// Build an integrity manifest from explicitly selected files. The caller must
/// authenticate/pin the source independently before trusting this manifest.
pub fn make_manifest(root: &Path, id: &str, paths: &[String]) -> Result<Manifest> {
    if id.trim().is_empty() || id.len() > 256 || paths.is_empty() || paths.len() > MAX_FILES {
        return Err("invalid pack ID or file count".into());
    }
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    let mut files = Vec::with_capacity(paths.len());
    let mut total = 0u64;
    let mut seen = BTreeSet::new();
    for path in paths {
        let relative = relative_path(path)?;
        if !seen.insert(path.to_ascii_lowercase()) {
            return Err("duplicate/case-colliding asset path".into());
        }
        let file = directory.open(relative)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err("asset must be a nonempty regular file".into());
        }
        total = total
            .checked_add(metadata.len())
            .ok_or("pack length overflow")?;
        if total > MAX_PACK_BYTES {
            return Err("pack exceeds client disk budget".into());
        }
        let (sha256, bytes) = digest_reader(BufReader::new(file), metadata.len())?;
        if bytes != metadata.len() {
            return Err("asset changed size while hashing".into());
        }
        files.push(Asset {
            path: path.clone(),
            bytes,
            sha256,
        });
    }
    Ok(Manifest {
        schema_version: 1,
        id: id.into(),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn portable_paths_reject_traversal_devices_ads_and_prefixes() {
        for path in [
            "../secret",
            "a/../secret",
            "/absolute",
            "C:/private",
            "a\\b",
            "CON.txt",
            "dir/LPT1",
            "a//b",
            "a/./b",
            "a.",
            "a ",
            "file:stream",
        ] {
            assert!(relative_path(path).is_err(), "{path}");
        }
        assert!(relative_path("onnx/model.data").is_ok());
    }
    #[test]
    fn streaming_hash_rejects_data_larger_than_declared() {
        assert!(digest_reader(Cursor::new(b"abc"), 2).is_err());
        assert_eq!(
            digest_reader(Cursor::new(b"abc"), 3).unwrap().0,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn impossible_read_limit_is_rejected_without_overflow() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("input");
        fs::write(&path, b"abc").unwrap();
        assert!(read_bounded(&path, u64::MAX).is_err());
    }
    #[test]
    fn manifest_verification_checks_size_hash_and_never_claims_publisher_trust() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("manifest.json");
        fs::write(root.path().join("fixture.data"), b"abc").unwrap();
        let manifest = json!({"schema_version":1,"id":"synthetic","files":[{"path":"fixture.data","bytes":3,"sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]});
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let result = verify(root.path(), &path).unwrap();
        assert!(result.integrity_verified);
        assert!(!result.publisher_authenticated);
        fs::write(root.path().join("fixture.data"), b"xyz").unwrap();
        assert!(verify(root.path(), &path).is_err());
    }
    #[test]
    fn generated_manifest_round_trips_without_loading_asset_into_memory() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("manifest.json");
        fs::write(root.path().join("fixture.data"), vec![7u8; 200_000]).unwrap();
        let manifest = make_manifest(root.path(), "pack", &["fixture.data".into()]).unwrap();
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert_eq!(verify(root.path(), &path).unwrap().total_bytes, 200_000);
    }

    #[cfg(unix)]
    #[test]
    fn directory_capability_rejects_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.data"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        assert!(make_manifest(root.path(), "escape", &["escape/secret.data".into()]).is_err());
    }
    #[test]
    fn manifest_rejects_case_collisions_and_scope_escape() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("manifest.json");
        let hash = "a".repeat(64);
        for files in [
            json!([{"path":"x","bytes":1,"sha256":hash},{"path":"X","bytes":1,"sha256":hash}]),
            json!([{"path":"../x","bytes":1,"sha256":hash}]),
        ] {
            fs::write(
                &path,
                serde_json::to_vec(&json!({"schema_version":1,"id":"bad","files":files})).unwrap(),
            )
            .unwrap();
            assert!(verify(root.path(), &path).is_err());
        }
    }
}
