//! Recover only parent-owned, bounded leases after obtaining supervisor.lock.
//! Never kill a recorded PID: it may have been reused by an unrelated process.
use crate::{Result, config::AppConfig};
use std::path::Path;

#[cfg(windows)]
pub fn validate_native_roots(config: &AppConfig, executable: &Path) -> Result<()> {
    let bin = executable
        .canonicalize()?
        .parent()
        .ok_or("executable directory missing")?
        .to_owned();
    let assets = config.asset_root.canonicalize()?;
    let state = config.state_dir.canonicalize()?;
    let key = config.llm_config.canonicalize()?;
    for root in [bin, assets] {
        if state.starts_with(&root) || root.starts_with(&state) || key.starts_with(&root) {
            return Err("native read roots overlap trusted state or credential configuration; use a separate bin/assets installation".into());
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn recover(config: &AppConfig, executable: &Path) -> Result<usize> {
    use nora_platform::sandbox::{SandboxLease, cleanup_lease};
    use std::{
        fs::{self, File},
        io::Read,
        os::windows::fs::MetadataExt,
    };
    let dir = config.state_dir.join("native-leases");
    if !dir.exists() {
        return Ok(0);
    }
    if fs::read_dir(&dir)?.next().is_none() {
        return Ok(0);
    }
    validate_native_roots(config, executable)?;
    let dir = dir.canonicalize()?;
    let scratch = config.state_dir.join("native-scratch").canonicalize()?;
    let bin = executable
        .canonicalize()?
        .parent()
        .ok_or("missing executable parent")?
        .to_owned();
    let assets = config.asset_root.canonicalize()?;
    let mut count = 0;
    let mut recovered = 0;
    for entry in fs::read_dir(&dir)? {
        count += 1;
        if count > 32 {
            return Err("native lease retention exceeded; inspection required".into());
        }
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_file()
            || meta.file_attributes() & 0x400 != 0
            || meta.len() > 65_536
            || path.extension().is_none_or(|s| s != "json")
        {
            return Err("invalid native lease file; no cleanup performed".into());
        }
        let mut bytes = Vec::new();
        File::open(&path)?.take(65_537).read_to_end(&mut bytes)?;
        if bytes.len() > 65_536 {
            return Err("native lease bound".into());
        }
        let lease: SandboxLease = serde_json::from_slice(&bytes)?;
        if lease.parent_pid == 0
            || lease.parent_creation_filetime == 0
            || lease.acl_paths.len() != 3
            || !lease.acl_paths.contains(&bin)
            || !lease.acl_paths.contains(&assets)
            || !lease.acl_paths.contains(&lease.scratch_dir)
            || lease.scratch_dir.parent() != Some(scratch.as_path())
            || lease
                .scratch_dir
                .file_name()
                .and_then(|s| s.to_str())
                .is_none_or(|s| !s.starts_with("audio-"))
        {
            return Err("native lease scope changed; inspection required".into());
        }
        match nora_platform::process_creation_filetime(lease.parent_pid) {
            Ok(identity) if identity == lease.parent_creation_filetime => {
                return Err("native lease owner is still running; recovery postponed".into());
            }
            Ok(_) => {}
            Err(e) if e.raw_os_error() == Some(87) => {}
            Err(_) => return Err("cannot establish native lease owner termination".into()),
        }
        // Platform revalidates SID, profile identity, every ancestor and all
        // reparse points before changing any ACL or removing the profile.
        cleanup_lease(&lease)?;
        if lease.scratch_dir.exists() {
            fs::remove_dir_all(&lease.scratch_dir)?;
        }
        fs::remove_file(path)?;
        recovered += 1;
    }
    Ok(recovered)
}
#[cfg(not(windows))]
pub fn recover(_config: &AppConfig, _executable: &Path) -> Result<usize> {
    Ok(0)
}
