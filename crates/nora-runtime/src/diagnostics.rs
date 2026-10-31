//! Explicit synthetic confinement checks. Never read real credentials or user
//! documents; all filesystem probes are newly created test-only sentinels.

use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Read,
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRequest {
    allowed_file: PathBuf,
    sentinel: PathBuf,
    denied_write: PathBuf,
    port: u16,
    result: PathBuf,
}

#[cfg(windows)]
pub fn sandbox_child_probe(request: &Path) -> Result<()> {
    if !nora_platform::sandbox::is_app_container()? {
        return Err("confinement probe must run inside an AppContainer".into());
    }
    let mut bytes = Vec::new();
    File::open(request)?.take(32_769).read_to_end(&mut bytes)?;
    if bytes.len() > 32_768 {
        return Err("probe request exceeds bound".into());
    }
    let request: ProbeRequest = serde_json::from_slice(&bytes)?;
    let allowed =
        fs::read_to_string(&request.allowed_file).is_ok_and(|s| s == "NORA_SYNTHETIC_ALLOWED");
    let denied = fs::read_to_string(&request.sentinel);
    let write = File::create_new(&request.denied_write);
    let address = SocketAddr::from(([127, 0, 0, 1], request.port));
    let network = TcpStream::connect_timeout(&address, Duration::from_secs(2));
    let report = json!({"app_container":true,"allowed_file_read":allowed,"outside_file_read_denied":denied.is_err(),"outside_read_os_error":denied.err().and_then(|e|e.raw_os_error()),"outside_write_denied":write.is_err(),"outside_write_os_error":write.err().and_then(|e|e.raw_os_error()),"localhost_connect_denied":network.is_err(),"network_os_error":network.err().and_then(|e|e.raw_os_error()),"test_environment_leaked":std::env::var_os("NORA_TEST_LEAK").is_some(),"llm_key_environment_present":std::env::var_os("NORA_LLM_API_KEY").is_some()});
    let file = File::create_new(request.result)?;
    serde_json::to_writer(&file, &report)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(windows)]
pub fn sandbox_check(executable: &Path, fixture_dir: &Path) -> Result<Value> {
    use nora_platform::sandbox::{SandboxRequest, spawn_with_lease};
    fs::create_dir_all(fixture_dir)?;
    let fixture = tempfile::Builder::new()
        .prefix("confinement-")
        .tempdir_in(fixture_dir)?;
    let scratch = tempfile::Builder::new()
        .prefix("worker-")
        .tempdir_in(fixture.path())?;
    let sentinel = fixture.path().join("outside-private.txt");
    fs::write(&sentinel, "NORA_SYNTHETIC_DENIED_NOT_A_CREDENTIAL")?;
    let denied_write = fixture.path().join("outside-write.txt");
    let allowed_file = scratch.path().join("allowed.txt");
    fs::write(&allowed_file, "NORA_SYNTHETIC_ALLOWED")?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let positive_network = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_ok();
    let parent_can_read = fs::read_to_string(&sentinel).is_ok();
    let result = scratch.path().join("probe-result.json");
    let request = ProbeRequest {
        allowed_file: allowed_file.canonicalize()?,
        sentinel: sentinel.canonicalize()?,
        denied_write: denied_write.clone(),
        port,
        result: result.clone(),
    };
    let request_path = scratch.path().join("probe.json");
    let file = File::create_new(&request_path)?;
    serde_json::to_writer(&file, &request)?;
    file.sync_all()?;
    drop(file);
    let executable = executable.canonicalize()?;
    let spec = SandboxRequest {
        read_roots: vec![
            executable
                .parent()
                .ok_or("probe executable has no parent")?
                .to_owned(),
        ],
        executable,
        args: vec![
            "--sandbox-probe".into(),
            request_path
                .to_str()
                .ok_or("probe path must be UTF-8")?
                .into(),
        ],
        work_dir: scratch.path().canonicalize()?,
        log_path: scratch.path().join("probe.log"),
    };
    let lease_path = fixture.path().join("lease.json");
    let mut child = spawn_with_lease(&spec, &mut |lease| {
        let temp = tempfile::NamedTempFile::new_in(fixture.path())?;
        serde_json::to_writer(&temp, lease).map_err(std::io::Error::other)?;
        temp.as_file().sync_all()?;
        temp.persist(&lease_path).map_err(|e| e.error)?;
        Ok(())
    })?;
    let pid = child.id();
    let exit = child.wait_timeout(Duration::from_secs(20))?;
    if exit != Some(0) {
        let _ = child.kill();
        return Err(
            "AppContainer probe did not complete; inspect its bounded synthetic log".into(),
        );
    }
    let storage_bytes = child.storage_bytes()?;
    let mut bytes = Vec::new();
    File::open(result)?.take(32_769).read_to_end(&mut bytes)?;
    if bytes.len() > 32_768 {
        return Err("probe result exceeds bound".into());
    }
    let mut report: Value = serde_json::from_slice(&bytes)?;
    let cleanup = child.finish();
    let passed = positive_network
        && parent_can_read
        && report["app_container"] == true
        && report["allowed_file_read"] == true
        && report["outside_file_read_denied"] == true
        && report["outside_write_denied"] == true
        && report["localhost_connect_denied"] == true
        && report["network_os_error"] == 10013
        && report["test_environment_leaked"] == false
        && report["llm_key_environment_present"] == false
        && !denied_write.exists()
        && cleanup.is_ok();
    report["pid"] = pid.into();
    report["parent_positive_file_control"] = parent_can_read.into();
    report["parent_positive_network_control"] = positive_network.into();
    report["profile_and_acl_cleanup_succeeded"] = cleanup.is_ok().into();
    report["native_storage_bytes"] = storage_bytes.into();
    report["passed"] = passed.into();
    if cleanup.is_ok() {
        fs::remove_file(lease_path)?;
    }
    if !passed {
        return Err(format!("confinement negative control failed: {report}").into());
    }
    Ok(report)
}

#[cfg(not(windows))]
pub fn sandbox_check(_executable: &Path, _fixture_dir: &Path) -> Result<Value> {
    Err("Windows AppContainer is required".into())
}

/// Bound logical installation/state footprint without following symlinks.
pub fn logical_disk_bytes(root: &Path, max_entries: usize) -> Result<u64> {
    let mut stack = vec![root.to_owned()];
    let mut entries = 0usize;
    let mut bytes = 0u64;
    while let Some(path) = stack.pop() {
        entries += 1;
        if entries > max_entries {
            return Err("disk accounting entry limit exceeded".into());
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err("disk accounting refuses symlinks".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("disk accounting refuses reparse points".into());
            }
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                stack.push(entry?.path());
            }
        } else if metadata.is_file() {
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or("disk accounting overflow")?;
        } else {
            return Err("unsupported installation entry".into());
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disk_accounting_counts_nested_files_and_enforces_entry_bounds() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/a"), b"12345").unwrap();
        assert_eq!(logical_disk_bytes(root.path(), 8).unwrap(), 5);
        assert!(logical_disk_bytes(root.path(), 2).is_err());
    }
}
