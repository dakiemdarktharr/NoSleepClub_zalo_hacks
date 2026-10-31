use crate::{BrokerError, Result, digest};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const MAX_FILE_BYTES: u64 = 65_536;
pub const MAX_LIST_ENTRIES: usize = 256;
const MAX_EXE_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn relative(value: &str, allow_empty: bool) -> Result<&Path> {
    if value.is_empty() && allow_empty {
        return Ok(Path::new("."));
    }
    if value.is_empty()
        || value.len() > 1024
        || value.contains(['\\', ':', '\0', '<', '>', '"', '|', '?', '*', '~'])
        || value.chars().any(|c| c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'))
    {
        return Err(BrokerError::Invalid("portable relative path"));
    }
    for part in value.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with([' ', '.'])
            || part.trim() != part
        {
            return Err(BrokerError::Invalid("path component"));
        }
        let upper = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if matches!(
            upper.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && (upper.len() == 4 && matches!(upper.as_bytes()[3], b'1'..=b'9')))
            || ["COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³"].contains(&upper.as_str())
        {
            return Err(BrokerError::Invalid("reserved device path"));
        }
        if sensitive_component(part) {
            return Err(BrokerError::PermissionDenied);
        }
    }
    let path = Path::new(value);
    if !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(BrokerError::Invalid("absolute or escaping path"));
    }
    Ok(path)
}

fn sensitive_component(part: &str) -> bool {
    let name = part.to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || name.starts_with("credentials.")
        || matches!(
            name.as_str(),
            ".git"
                | ".ssh"
                | ".aws"
                | ".azure"
                | ".gnupg"
                | ".codex"
                | ".tools"
                | ".artifacts"
                | "target"
                | "credentials"
                | "credentials.json"
                | "id_rsa"
                | "id_ed25519"
                | ".npmrc"
                | ".netrc"
                | ".pypirc"
                | ".config"
                | ".local"
                | ".cache"
                | ".nora"
                | "nora-state"
                | "nora_state"
                | "appdata"
                | "login data"
                | "cookies"
                | "local state"
        )
        || [
            ".key",
            ".pem",
            ".p12",
            ".pfx",
            ".kdbx",
            ".sqlite",
            ".sqlite3",
            ".db",
            ".db-wal",
            ".db-shm",
            ".sqlite-wal",
            ".sqlite-shm",
        ]
        .iter()
        .any(|ext| name.ends_with(ext))
}

pub(crate) fn open_root(path: &Path) -> Result<(PathBuf, Dir)> {
    let canonical = fs::canonicalize(path)?;
    if !canonical.is_dir() {
        return Err(BrokerError::Invalid("root directory"));
    }
    Ok((
        canonical.clone(),
        Dir::open_ambient_dir(canonical, ambient_authority())?,
    ))
}

/// Check all existing components for links. The cap-std open remains the actual
/// containment boundary if the namespace changes after these checks. An OS
/// account able to mutate granted files is not treated as an isolated adversary.
fn no_links(dir: &Dir, path: &Path, final_may_be_missing: bool) -> Result<()> {
    let mut current = PathBuf::new();
    let components: Vec<_> = path.components().collect();
    for (index, part) in components.iter().enumerate() {
        current.push(part.as_os_str());
        match dir.symlink_metadata(&current) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(BrokerError::PermissionDenied);
                }
                if index + 1 < components.len() && !meta.is_dir() {
                    return Err(BrokerError::Invalid("parent directory"));
                }
            }
            Err(error)
                if final_may_be_missing
                    && index + 1 == components.len()
                    && error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub(crate) fn read_text(dir: &Dir, path: &str) -> Result<(String, String, u64)> {
    let relative = relative(path, false)?;
    no_links(dir, relative, false)?;
    let file = dir.open(relative)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err(BrokerError::Limit("text file size/type"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(BrokerError::Limit("growing text file"));
    }
    let hash = digest(&bytes);
    let count = bytes.len() as u64;
    let text = String::from_utf8(bytes).map_err(|_| BrokerError::Invalid("UTF-8 text"))?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(BrokerError::Invalid("binary/control text"));
    }
    Ok((text, hash, count))
}

pub(crate) fn list(dir: &Dir, path: &str) -> Result<Value> {
    let relative = relative(path, true)?;
    no_links(dir, relative, false)?;
    let child = dir.open_dir(relative)?;
    let mut entries = Vec::new();
    let mut scanned = 0usize;
    for entry in child.entries()?.take(MAX_LIST_ENTRIES + 1) {
        scanned += 1;
        if scanned > MAX_LIST_ENTRIES {
            break;
        }
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if self::relative(&name, false).is_err() {
            continue;
        }
        let metadata = entry.metadata()?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let kind = if metadata.is_dir() {
            "directory"
        } else if metadata.is_file() {
            "file"
        } else {
            continue;
        };
        entries.push(json!({"name":name,"kind":kind,"bytes":metadata.len()}));
    }
    entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!({"entries":entries,"truncated":scanned > MAX_LIST_ENTRIES}))
}

pub(crate) fn search(dir: &Dir, path: &str, query: &str) -> Result<Value> {
    let (text, hash, count) = read_text(dir, path)?;
    let mut matches = Vec::new();
    let mut truncated = false;
    for (index, line) in text.lines().enumerate() {
        if line.contains(query) {
            if matches.len() == 20 {
                truncated = true;
                break;
            }
            let snippet: String = line.chars().take(1024).collect();
            matches.push(
                json!({"line":index+1,"text":snippet,"line_truncated":line.chars().count()>1024}),
            );
        }
    }
    Ok(
        json!({"matches":matches,"truncated":truncated,"sha256":hash,"bytes":count,"untrusted_content":true}),
    )
}

pub(crate) fn check_create(dir: &Dir, path: &str) -> Result<()> {
    let path = relative(path, false)?;
    no_links(dir, path, true)?;
    match dir.symlink_metadata(path) {
        Ok(_) => Err(BrokerError::TargetExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn create_note(dir: &Dir, path: &str, content: &str) -> Result<Value> {
    check_create(dir, path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = dir
        .open_with(relative(path, false)?, &options)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                BrokerError::TargetExists
            } else {
                BrokerError::Io(e)
            }
        })?;
    // Failure after this point must leave the effect uncertain. Never remove or
    // retry the path automatically: the target may already be visible.
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);
    let (_, actual, bytes) = read_text(dir, path)?;
    let expected = digest(content.as_bytes());
    if actual != expected || bytes != content.len() as u64 {
        return Err(BrokerError::NeedsReconciliation);
    }
    Ok(json!({"path":path,"sha256":actual,"bytes":bytes,"readback_verified":true}))
}

pub(crate) fn open_executable(path: &Path) -> Result<(PathBuf, File, String)> {
    let canonical = fs::canonicalize(path)?;
    if canonical
        .extension()
        .and_then(|x| x.to_str())
        .is_none_or(|x| !x.eq_ignore_ascii_case("exe"))
    {
        return Err(BrokerError::Invalid("Windows .exe executable"));
    }
    let name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if [
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "wscript.exe",
        "cscript.exe",
        "mshta.exe",
        "rundll32.exe",
        "regsvr32.exe",
        "python.exe",
        "pythonw.exe",
        "node.exe",
        "bash.exe",
        "sh.exe",
        "wsl.exe",
    ]
    .contains(&name.as_str())
    {
        return Err(BrokerError::PermissionDenied);
    }
    #[cfg(windows)]
    {
        use std::path::Prefix;
        if canonical.components().any(|c| matches!(c, Component::Prefix(p) if matches!(p.kind(),Prefix::UNC(..)|Prefix::VerbatimUNC(..)|Prefix::DeviceNS(..)))) {
            return Err(BrokerError::PermissionDenied);
        }
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut file = options.open(&canonical)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() < 2 || meta.len() > MAX_EXE_BYTES {
        return Err(BrokerError::Limit("executable size/type"));
    }
    let mut signature = [0u8; 2];
    file.read_exact(&mut signature)?;
    if &signature != b"MZ" {
        return Err(BrokerError::Invalid("PE executable header"));
    }
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = [0u8; 65_536];
    let mut hash = sha2::Sha256::new();
    use sha2::Digest;
    let mut count = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or(BrokerError::Limit("executable size"))?;
        if count > MAX_EXE_BYTES {
            return Err(BrokerError::Limit("growing executable"));
        }
        hash.update(&buffer[..n]);
    }
    Ok((canonical, file, format!("{:x}", hash.finalize())))
}
