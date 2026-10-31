//! Windows native-audio confinement. A fresh, zero-capability AppContainer SID
//! gets RX only on explicitly staged binary/model roots and M on one scratch
//! directory. The worker inherits only NUL input and its diagnostic log handles,
//! starts suspended, enters a one-process/1.7GB job, then resumes. There is no
//! unsandboxed fallback. The parent must meter scratch/profile storage and keep
//! the returned lease in trusted state for cleanup after a parent crash.
//!
//! This is original Win32 FFI, not an assurance that native parsers are safe or
//! that AppContainer defeats kernel/driver vulnerabilities. No network capability
//! or loopback exemption is requested or changed.

use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io,
    mem::{size_of, size_of_val, zeroed},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Component, Path, PathBuf, Prefix},
    process::{Command, Stdio},
    ptr, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, LocalFree, SetHandleInformation, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    Security::{
        Authorization::ConvertSidToStringSidW,
        FreeSid, GetTokenInformation,
        Isolation::{
            CreateAppContainerProfile, DeleteAppContainerProfile,
            DeriveAppContainerSidFromAppContainerName, GetAppContainerFolderPath,
        },
        PSID, SECURITY_CAPABILITIES, TOKEN_QUERY, TokenIsAppContainer,
    },
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
            JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        SystemInformation::{GetSystemDirectoryW, GetWindowsDirectoryW},
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
            GetCurrentProcessId, GetExitCodeProcess, InitializeProcThreadAttributeList,
            LPPROC_THREAD_ATTRIBUTE_LIST, OpenProcessToken, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread,
            STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
            WaitForSingleObject,
        },
    },
};

pub const AUDIO_COMMIT_LIMIT_BYTES: usize = 1_700_000_000;
pub const MAX_WORKER_LOG_BYTES: u64 = 65_536;
pub const MAX_WORKER_STORAGE_BYTES: u64 = 32_000_000;
const PROFILE_PREFIX: &str = "NoSleepClub.NORA.Audio.";
const KILL_CODE: u32 = 0x4e4f_5241;

#[derive(Debug, Clone)]
pub struct SandboxRequest {
    pub executable: PathBuf,
    pub args: Vec<String>,
    /// Staged directories only, never a project/user profile or config root.
    pub read_roots: Vec<PathBuf>,
    /// A newly created, parent-owned directory with only this request's inputs.
    pub work_dir: PathBuf,
    pub log_path: PathBuf,
}

/// Persist only outside worker-accessible directories. Never trust a lease
/// supplied by the worker. Unique names/SIDs are checked again during cleanup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxLease {
    pub profile_name: String,
    pub sid: String,
    pub acl_paths: Vec<PathBuf>,
    pub scratch_dir: PathBuf,
    pub profile_folder: Option<PathBuf>,
    pub parent_pid: u32,
    pub parent_creation_filetime: u64,
    pub created_unix_ms: u64,
}

pub struct SandboxChild {
    process: Option<OwnedHandle>,
    job: Option<OwnedHandle>,
    log: File,
    pid: u32,
    lease: Lease,
}

impl SandboxChild {
    pub fn spawn(request: &SandboxRequest) -> io::Result<Self> {
        spawn(request)
    }
    pub fn id(&self) -> u32 {
        self.pid
    }
    pub fn lease_metadata(&self) -> SandboxLease {
        self.lease.metadata.clone()
    }

    /// Bounded logical file-byte meter for both explicitly granted scratch and
    /// Windows' implicit package storage. Reparse points are rejected rather than
    /// followed by the trusted parent. This is a watchdog, not a disk quota.
    pub fn storage_bytes(&self) -> io::Result<u64> {
        let mut roots = vec![self.lease.metadata.scratch_dir.clone()];
        if let Some(path) = self.lease.metadata.profile_folder.as_ref() {
            roots.push(path.clone());
        }
        storage_bytes(&roots)
    }

    pub fn try_wait(&mut self) -> io::Result<Option<u32>> {
        if self.log.metadata()?.len() > MAX_WORKER_LOG_BYTES {
            self.kill()?;
            return Err(io::Error::other(
                "native worker diagnostics exceeded their bound",
            ));
        }
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| io::Error::other("native worker is already closed"))?;
        wait_process(process, 0)
    }

    pub fn wait_timeout(&mut self, timeout: Duration) -> io::Result<Option<u32>> {
        let start = Instant::now();
        loop {
            if let Some(code) = self.try_wait()? {
                return Ok(Some(code));
            }
            if start.elapsed() >= timeout {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(10).min(timeout.saturating_sub(start.elapsed())));
        }
    }

    pub fn kill(&mut self) -> io::Result<()> {
        let Some(process) = self.process.as_ref() else {
            return Ok(());
        };
        if wait_process(process, 0)?.is_some() {
            return Ok(());
        }
        // SAFETY: self owns a live job handle. Termination affects only its
        // explicitly assigned native worker; no user application enters this job.
        let job = self
            .job
            .as_ref()
            .ok_or_else(|| io::Error::other("native worker job is already closed"))?;
        if unsafe { TerminateJobObject(job.as_raw_handle(), KILL_CODE) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Explicit teardown reports ACL/profile cleanup failures. Drop remains a
    /// best-effort emergency path; release checks should call this method.
    pub fn finish(mut self) -> io::Result<()> {
        self.kill()?;
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| io::Error::other("native worker is already closed"))?;
        if wait_process(process, 5000)?.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "native worker did not terminate",
            ));
        }
        // Release terminated process/token/job references before deleting its
        // package profile; registry/storage may remain in-use while handles live.
        drop(self.process.take());
        drop(self.job.take());
        cleanup_lease_with_system(&self.lease.metadata, &self.lease.system_dir)?;
        self.lease.cleanup_on_drop = false;
        Ok(())
    }
}

impl Drop for SandboxChild {
    fn drop(&mut self) {
        let _ = self.kill();
        // Confirm exit before revoking ACLs/deleting the profile. If the OS
        // cannot confirm exit, the job handle still kills on close but cleanup
        // is left to the trusted startup lease scavenger rather than racing it.
        if let Some(process) = self.process.as_ref()
            && !matches!(wait_process(process, 5000), Ok(Some(_)))
        {
            self.lease.cleanup_on_drop = false;
        }
    }
}

pub fn is_app_container() -> io::Result<bool> {
    // SAFETY: GetCurrentProcess returns a borrowed pseudo handle; no closing is
    // needed and token inspection requests only TOKEN_QUERY.
    token_is_app_container(unsafe { GetCurrentProcess() })
}

fn token_is_app_container(process: HANDLE) -> io::Result<bool> {
    let mut raw = ptr::null_mut();
    // SAFETY: current-process pseudo handle is borrowed, TOKEN_QUERY is minimum
    // access, and a successful call returns one newly owned token handle.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcessToken succeeded and transferred ownership of raw.
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut value = 0u32;
    let mut written = 0u32;
    // SAFETY: TokenIsAppContainer returns a DWORD into an exact-size writable
    // buffer; the handle has TOKEN_QUERY and remains alive during the call.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenIsAppContainer,
            (&mut value as *mut u32).cast(),
            size_of::<u32>() as u32,
            &mut written,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if written != size_of::<u32>() as u32 {
        return Err(io::Error::other("invalid AppContainer token information"));
    }
    Ok(value != 0)
}

pub fn spawn(request: &SandboxRequest) -> io::Result<SandboxChild> {
    spawn_with_lease(request, &mut |_| Ok(()))
}

/// The trusted parent atomically upserts metadata outside all worker-accessible
/// roots. The hook can run more than once: planned SID/scope before profile
/// creation; profile storage after creation; final scope before untrusted code.
/// The runtime must use this entry point rather than the no-op convenience API.
pub fn spawn_with_lease(
    request: &SandboxRequest,
    persist: &mut dyn FnMut(&SandboxLease) -> io::Result<()>,
) -> io::Result<SandboxChild> {
    let (system_dir, windows_dir) = system_directories()?;
    let checked = validate_request(request)?;
    let (lease, sid) = Lease::create(
        system_dir.clone(),
        &checked.read_roots,
        &checked.work_dir,
        persist,
    )?;
    for root in &checked.read_roots {
        lease.grant(root, "(OI)(CI)(RX)")?;
    }
    lease.grant(&checked.work_dir, "(OI)(CI)(M)")?;
    run_icacls(
        &system_dir,
        &checked.work_dir,
        "/setintegritylevel",
        "(OI)(CI)L",
    )?;
    persist(&lease.metadata)?;

    // A pinned log handle, not merely its path, is retained for the bound check.
    // Read-only sharing blocks replacement/truncation while the worker is alive.
    let log = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(1)
        .open(&checked.log_path)?;
    let null_input = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open("NUL")?;
    let mut inheritance =
        HandleInheritance::enable([log.as_raw_handle(), null_input.as_raw_handle()])?;
    let handles = [null_input.as_raw_handle(), log.as_raw_handle()];
    let capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: sid.raw,
        Capabilities: ptr::null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    let mut attributes = Attributes::new()?;
    // SAFETY: these initialized buffers and their referenced SID/handles remain
    // alive and stationary through the later CreateProcessW call.
    unsafe {
        attributes.set(
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            (&capabilities as *const SECURITY_CAPABILITIES).cast(),
            size_of::<SECURITY_CAPABILITIES>(),
        )?;
        attributes.set(
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_ptr().cast(),
            size_of_val(&handles),
        )?;
    }
    // SAFETY: STARTUPINFOEXW is a Win32 POD struct; cb and all borrowed pointers
    // below are initialized before CreateProcess and outlive that synchronous call.
    let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = null_input.as_raw_handle();
    startup.StartupInfo.hStdOutput = log.as_raw_handle();
    startup.StartupInfo.hStdError = log.as_raw_handle();
    startup.lpAttributeList = attributes.raw;
    let executable = wide(checked.executable.as_os_str())?;
    let working = wide(checked.work_dir.as_os_str())?;
    let mut command = command_line(&checked.executable, &request.args)?;
    let environment = environment_block(&windows_dir, &system_dir, &checked.work_dir)?;
    let job = native_job()?;
    // SAFETY: native output struct supports a zero initial state.
    let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
    // SAFETY: all UTF-16 pointers are nul-terminated and live throughout the
    // call; mutable command line is uniquely borrowed. Attribute storage and
    // SID/capability/handle arrays stay alive. Only the two listed handles can
    // be inherited. The environment is a complete double-nul-terminated block
    // with no API keys. Suspended creation prevents execution before job setup.
    if unsafe {
        CreateProcessW(
            executable.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr().cast(),
            working.as_ptr(),
            &startup.StartupInfo,
            &mut info,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if info.hProcess.is_null() || info.hThread.is_null() {
        // SAFETY: defensively clean any non-null handles from an invalid success
        // response. Only this just-created suspended process can be affected.
        unsafe {
            if !info.hProcess.is_null() {
                TerminateProcess(info.hProcess, KILL_CODE);
                WaitForSingleObject(info.hProcess, 5000);
                CloseHandle(info.hProcess);
            }
            if !info.hThread.is_null() {
                CloseHandle(info.hThread);
            }
        }
        return Err(io::Error::other("invalid native process handles"));
    }
    // SAFETY: successful CreateProcessW returned new process and thread handles.
    let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
    // SAFETY: thread handle is newly owned and distinct from process handle.
    let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
    let mut pending = PendingProcess {
        process: Some(process),
        thread,
    };
    inheritance.disable()?;
    let process = pending.process.as_ref().expect("owned process");
    // SAFETY: both handles are live. The worker's primary thread is suspended;
    // assignment to this nested limited job happens before any worker code runs.
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // Verify the kernel token in the trusted parent before untrusted code runs;
    // do not accept a worker's self-reported isolation status as proof.
    if !token_is_app_container(process.as_raw_handle())? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native worker did not receive an AppContainer token",
        ));
    }
    // SAFETY: the newly created primary thread is suspended once and owned here.
    if unsafe { ResumeThread(pending.thread.as_raw_handle()) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(SandboxChild {
        process: pending.process.take(),
        job: Some(job),
        log,
        pid: info.dwProcessId,
        lease,
    })
}

struct CheckedRequest {
    executable: PathBuf,
    read_roots: Vec<PathBuf>,
    work_dir: PathBuf,
    log_path: PathBuf,
}

fn validate_request(request: &SandboxRequest) -> io::Result<CheckedRequest> {
    if request.args.len() > 16 || request.read_roots.is_empty() || request.read_roots.len() > 8 {
        return Err(invalid("native request count limit"));
    }
    if request
        .args
        .iter()
        .any(|s| s.len() > 8192 || s.contains('\0'))
        || request.args.iter().map(String::len).sum::<usize>() > 16_384
    {
        return Err(invalid("native argument limit"));
    }
    let executable = fs::canonicalize(&request.executable)?;
    if !fs::metadata(&executable)?.is_file()
        || executable
            .extension()
            .is_none_or(|s| !s.eq_ignore_ascii_case("exe"))
    {
        return Err(invalid("native executable"));
    }
    require_local(&executable)?;
    let work_dir = fs::canonicalize(&request.work_dir)?;
    if !work_dir.is_dir()
        || fs::symlink_metadata(&request.work_dir)?
            .file_type()
            .is_symlink()
    {
        return Err(invalid("native scratch directory"));
    }
    require_local(&work_dir)?;
    let mut scratch_bytes = 0u64;
    for (index, entry) in fs::read_dir(&work_dir)?.enumerate() {
        if index >= 8 {
            return Err(invalid("native scratch is not a fresh request directory"));
        }
        let entry = entry?;
        let meta = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !meta.is_file()
            || entry.file_type()?.is_symlink()
            || name.starts_with(".env")
            || name.starts_with("credentials")
            || name.ends_with(".key")
            || name.ends_with(".pem")
        {
            return Err(invalid(
                "native scratch must contain only this request's inputs",
            ));
        }
        scratch_bytes = scratch_bytes
            .checked_add(meta.len())
            .ok_or_else(|| invalid("native scratch size"))?;
        if scratch_bytes > 32_000_000 {
            return Err(invalid("native scratch input size"));
        }
    }
    let mut read_roots = Vec::new();
    for root in &request.read_roots {
        let canonical = fs::canonicalize(root)?;
        require_local(&canonical)?;
        if !canonical.is_dir()
            || canonical.parent().is_none_or(|p| p.parent().is_none())
            || fs::symlink_metadata(root)?.file_type().is_symlink()
            || canonical.starts_with(&work_dir)
            || work_dir.starts_with(&canonical)
        {
            return Err(invalid(
                "native read root must be a separate staged directory",
            ));
        }
        for forbidden in [
            ".env",
            ".env.local",
            ".git",
            ".ssh",
            ".aws",
            ".codex",
            "broker.sqlite",
            "goals.sqlite",
        ] {
            if canonical.join(forbidden).exists() {
                return Err(invalid(
                    "native read root contains application state or credentials",
                ));
            }
        }
        if !read_roots.contains(&canonical) {
            read_roots.push(canonical);
        }
    }
    if !read_roots.iter().any(|root| executable.starts_with(root)) {
        return Err(invalid("native executable is outside staged read roots"));
    }
    let log_name = request
        .log_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| invalid("native log name"))?;
    if log_name.is_empty()
        || log_name.len() > 128
        || !log_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || log_name.ends_with('.')
        || log_name.starts_with('.')
        || ["CON", "PRN", "AUX", "NUL"].contains(
            &log_name
                .split('.')
                .next()
                .unwrap_or("")
                .to_ascii_uppercase()
                .as_str(),
        )
    {
        return Err(invalid("native log name"));
    }
    let log_parent = request
        .log_path
        .parent()
        .ok_or_else(|| invalid("native log path"))?;
    if fs::canonicalize(log_parent)? != work_dir {
        return Err(invalid(
            "native log must be in this request scratch directory",
        ));
    }
    let log_path = work_dir.join(log_name);
    if log_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "native diagnostic log already exists",
        ));
    }
    Ok(CheckedRequest {
        executable,
        read_roots,
        work_dir,
        log_path,
    })
}

fn require_local(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.as_os_str().len()>4096 || path.components().any(|part|matches!(part,Component::Prefix(p) if !matches!(p.kind(),Prefix::Disk(..)|Prefix::VerbatimDisk(..)))) {
        return Err(invalid("native paths must be bounded local paths"));
    }
    Ok(())
}

struct Sid {
    raw: PSID,
}
impl Drop for Sid {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            /* SAFETY: profile/derive API allocated this SID and transferred ownership. */
            unsafe { FreeSid(self.raw) };
        }
    }
}

struct Lease {
    metadata: SandboxLease,
    system_dir: PathBuf,
    cleanup_on_drop: bool,
}
impl Lease {
    fn create(
        system_dir: PathBuf,
        read_roots: &[PathBuf],
        scratch: &Path,
        persist: &mut dyn FnMut(&SandboxLease) -> io::Result<()>,
    ) -> io::Result<(Self, Sid)> {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| io::Error::other("Windows entropy unavailable"))?;
        let mut name = String::from(PROFILE_PREFIX);
        for byte in nonce {
            use std::fmt::Write;
            write!(name, "{byte:02x}").expect("String formatting");
        }
        let name_wide = wide(OsStr::new(&name))?;
        let mut raw = ptr::null_mut();
        // SAFETY: deriving a SID from this fresh product name does not create a
        // profile. Persisting its scope before creation closes the crash gap.
        let status =
            unsafe { DeriveAppContainerSidFromAppContainerName(name_wide.as_ptr(), &mut raw) };
        if status < 0 {
            return Err(io::Error::other("cannot derive new AppContainer SID"));
        }
        let sid = Sid { raw };
        if sid.raw.is_null() {
            return Err(io::Error::other("AppContainer profile returned no SID"));
        }
        let sid_text = sid_string(&sid)?;
        let created_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("system time"))?
            .as_millis()
            .try_into()
            .map_err(|_| invalid("system time"))?;
        // SAFETY: current process ID is a scalar with no ownership semantics.
        let parent_pid = unsafe { GetCurrentProcessId() };
        let mut acl_paths = read_roots.to_vec();
        acl_paths.push(scratch.to_path_buf());
        let mut lease = Self {
            metadata: SandboxLease {
                profile_name: name,
                sid: sid_text,
                acl_paths,
                scratch_dir: scratch.to_path_buf(),
                profile_folder: None,
                parent_pid,
                parent_creation_filetime: crate::process_creation_filetime(parent_pid)?,
                created_unix_ms,
            },
            system_dir,
            cleanup_on_drop: false,
        };
        persist(&lease.metadata)?;
        let display = wide(OsStr::new("NORA isolated audio"))?;
        let mut created_raw = ptr::null_mut();
        // SAFETY: bounded nul-terminated product name and a null/zero capability
        // list. No existing profile is reused and no network exemption is set.
        let status = unsafe {
            CreateAppContainerProfile(
                name_wide.as_ptr(),
                display.as_ptr(),
                display.as_ptr(),
                ptr::null(),
                0,
                &mut created_raw,
            )
        };
        if status < 0 {
            return Err(io::Error::other(format!(
                "AppContainer profile creation failed (HRESULT {status:#x})"
            )));
        }
        lease.cleanup_on_drop = true;
        let created_sid = Sid { raw: created_raw };
        if sid_string(&created_sid)? != lease.metadata.sid {
            return Err(io::Error::other("created AppContainer SID mismatch"));
        }
        lease.metadata.profile_folder = Some(profile_folder(&lease.metadata.sid)?);
        persist(&lease.metadata)?;
        Ok((lease, sid))
    }
    fn grant(&self, path: &Path, rights: &str) -> io::Result<()> {
        let entry = format!("*{}:{rights}", self.metadata.sid);
        run_icacls(&self.system_dir, path, "/grant", &entry)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            let _ = cleanup_lease_with_system(&self.metadata, &self.system_dir);
        }
    }
}

/// Trusted startup cleanup only. The parent must ensure the recorded owning
/// process is dead; this function never guesses whether another session is live.
pub fn cleanup_lease(metadata: &SandboxLease) -> io::Result<()> {
    let (system_dir, _) = system_directories()?;
    cleanup_lease_with_system(metadata, &system_dir)
}

fn cleanup_lease_with_system(metadata: &SandboxLease, system_dir: &Path) -> io::Result<()> {
    if metadata.acl_paths.len() > 9
        || !valid_profile_name(&metadata.profile_name)
        || !metadata.acl_paths.contains(&metadata.scratch_dir)
    {
        return Err(invalid("sandbox lease identity"));
    }
    let name = wide(OsStr::new(&metadata.profile_name))?;
    let mut raw = ptr::null_mut();
    // SAFETY: bounded nul-terminated product profile name; success allocates SID.
    let status = unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut raw) };
    if status < 0 {
        return Err(io::Error::other("cannot derive cleanup SID"));
    }
    let sid = Sid { raw };
    if sid_string(&sid)? != metadata.sid {
        return Err(invalid("sandbox lease SID mismatch"));
    }
    // The worker has exited, but its filesystem output is still untrusted.
    // Validate the entire cleanup scope BEFORE any inheritable ACL mutation;
    // never let the trusted icacls/profile cleaner follow a planted junction.
    let mut scopes = metadata.acl_paths.clone();
    if let Some(folder) = metadata.profile_folder.as_ref() {
        scopes.push(folder.clone());
    }
    for path in &scopes {
        require_local(path)?;
        match fs::symlink_metadata(path) {
            Ok(_) => {
                let resolved = fs::canonicalize(path)?;
                if windows_path_key(&resolved) != windows_path_key(path) {
                    return Err(invalid("native cleanup path was retargeted"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    scan_storage(&scopes, 2048, 16, u64::MAX)?;
    let mut first_error = None;
    for path in metadata.acl_paths.iter().rev() {
        require_local(path)?;
        if path.exists()
            && let Err(error) =
                run_icacls(system_dir, path, "/remove:g", &format!("*{}", metadata.sid))
        {
            first_error.get_or_insert(error);
        }
    }
    require_local(&metadata.scratch_dir)?;
    if metadata.scratch_dir.exists()
        && let Err(error) = run_icacls(
            system_dir,
            &metadata.scratch_dir,
            "/setintegritylevel",
            "(OI)(CI)M",
        )
    {
        first_error.get_or_insert(error);
    }
    // SAFETY: the validated unique profile belongs to this lease. No global
    // network/loopback state is changed. Cleanup failure is returned, not hidden.
    let status = unsafe { DeleteAppContainerProfile(name.as_ptr()) };
    if status < 0 && status != 0x8007_0002_u32 as i32 && status != 0x8007_0003_u32 as i32 {
        first_error.get_or_insert_with(|| {
            io::Error::other(format!(
                "AppContainer profile cleanup failed (HRESULT {status:#x})"
            ))
        });
    }
    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(())
    }
}

fn valid_profile_name(name: &str) -> bool {
    name.strip_prefix(PROFILE_PREFIX)
        .is_some_and(|nonce| nonce.len() == 32 && nonce.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn sid_string(sid: &Sid) -> io::Result<String> {
    if sid.raw.is_null() {
        return Err(invalid("empty profile SID"));
    }
    let mut text = ptr::null_mut();
    // SAFETY: SID is allocated/owned and valid; API allocates a nul-terminated
    // string through text, which must be released with LocalFree.
    if unsafe { ConvertSidToStringSidW(sid.raw, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if text.is_null() {
        return Err(io::Error::other("empty SID string"));
    }
    let mut length = 0usize;
    // SAFETY: SID strings from this API are valid nul-terminated allocations.
    // Bound iteration to256 UTF-16 code units, far above a textual SID's length.
    while length < 256 && unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    let result = if length == 256 {
        Err(io::Error::other("excessive SID string"))
    }
    // SAFETY: the API allocation contains length initialized code units.
    else {
        String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) })
            .map_err(|_| io::Error::other("invalid SID string"))
    };
    // SAFETY: text was allocated by ConvertSidToStringSidW.
    unsafe { LocalFree(text.cast()) };
    result
}

fn profile_folder(sid: &str) -> io::Result<PathBuf> {
    let sid = wide(OsStr::new(sid))?;
    let mut raw = ptr::null_mut();
    // SAFETY: nul-terminated SID string and writable output pointer. On success
    // Windows allocates a nul-terminated path with CoTaskMemAlloc ownership.
    let status = unsafe { GetAppContainerFolderPath(sid.as_ptr(), &mut raw) };
    if status < 0 {
        return Err(io::Error::other(
            "cannot query AppContainer profile storage",
        ));
    }
    if raw.is_null() {
        return Err(io::Error::other("AppContainer profile storage is empty"));
    }
    let mut length = 0usize;
    // SAFETY: Windows guarantees a valid nul-terminated UTF-16 allocation. The
    // bound is Windows' maximum path length, not a guessed allocation length.
    while length < 32_768 && unsafe { *raw.add(length) } != 0 {
        length += 1;
    }
    let result = if length == 32_768 {
        Err(invalid("AppContainer profile storage path length"))
    } else {
        // SAFETY: length initialized code units precede the API string's nul.
        let value = OsString::from_wide(unsafe { std::slice::from_raw_parts(raw, length) });
        let path = PathBuf::from(value);
        require_local(&path).map(|()| path)
    };
    // SAFETY: GetAppContainerFolderPath transfers a CoTaskMem-allocated buffer.
    unsafe { CoTaskMemFree(raw.cast()) };
    result
}

struct HandleInheritance {
    handles: [HANDLE; 2],
    enabled: bool,
}
impl HandleInheritance {
    fn enable(handles: [HANDLE; 2]) -> io::Result<Self> {
        let mut guard = Self {
            handles,
            enabled: true,
        };
        for handle in handles {
            // SAFETY: caller borrows two owned valid Files for the lifetime of
            // this local guard. It never closes these handles, only their flag.
            if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) }
                == 0
            {
                let error = io::Error::last_os_error();
                let _ = guard.disable();
                return Err(error);
            }
        }
        Ok(guard)
    }
    fn disable(&mut self) -> io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let mut first_error = None;
        for handle in self.handles {
            // SAFETY: the still-owned File handles outlive the guard, and flags
            // are parent handle-table attributes; child copies are unaffected.
            if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) } == 0 {
                first_error.get_or_insert_with(io::Error::last_os_error);
            }
        }
        if let Some(error) = first_error {
            Err(error)
        } else {
            self.enabled = false;
            Ok(())
        }
    }
}
impl Drop for HandleInheritance {
    fn drop(&mut self) {
        let _ = self.disable();
    }
}

struct Attributes {
    storage: Vec<usize>,
    raw: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    fn new() -> io::Result<Self> {
        let mut bytes = 0usize;
        // SAFETY: documented size-query form passes null with writable size.
        unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes) };
        if bytes == 0 || bytes > 65_536 {
            return Err(io::Error::other("invalid process attribute size"));
        }
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let raw = storage.as_mut_ptr().cast();
        // SAFETY: Vec<usize> supplies pointer alignment and at least bytes writable
        // storage. It is never resized while the native attribute list exists.
        if unsafe { InitializeProcThreadAttributeList(raw, 2, 0, &mut bytes) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { storage, raw })
    }
    unsafe fn set(
        &mut self,
        attribute: usize,
        value: *const std::ffi::c_void,
        bytes: usize,
    ) -> io::Result<()> {
        // SAFETY: callers pass live initialized SECURITY_CAPABILITIES/handle
        // arrays with matching sizes; those buffers outlive CreateProcessW.
        if unsafe {
            UpdateProcThreadAttribute(
                self.raw,
                0,
                attribute,
                value,
                bytes,
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: raw points into still-live self.storage initialized by the API.
        unsafe { DeleteProcThreadAttributeList(self.raw) };
        let _ = &self.storage;
    }
}

struct PendingProcess {
    process: Option<OwnedHandle>,
    thread: OwnedHandle,
}
impl Drop for PendingProcess {
    fn drop(&mut self) {
        if let Some(process) = self.process.as_ref() {
            // SAFETY: this guard owns a newly created/suspended process which
            // failed setup. It is terminated and waited before handles close.
            unsafe {
                TerminateProcess(process.as_raw_handle(), KILL_CODE);
                WaitForSingleObject(process.as_raw_handle(), 5000);
            }
        }
    }
}

fn native_job() -> io::Result<OwnedHandle> {
    // SAFETY: unnamed, non-inheritable job creation; successful handle is owned.
    let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful CreateJobObjectW transferred this handle.
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };
    // SAFETY: zero is a valid initial POD state before required fields are set.
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_JOB_MEMORY
        | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
    info.BasicLimitInformation.ActiveProcessLimit = 1;
    info.JobMemoryLimit = AUDIO_COMMIT_LIMIT_BYTES;
    // SAFETY: matching information class/initialized struct/exact byte size.
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

fn wait_process(process: &OwnedHandle, milliseconds: u32) -> io::Result<Option<u32>> {
    // SAFETY: handle is owned and has SYNCHRONIZE access from CreateProcessW.
    match unsafe { WaitForSingleObject(process.as_raw_handle(), milliseconds) } {
        WAIT_TIMEOUT => Ok(None),
        WAIT_OBJECT_0 => {
            let mut code = 0u32;
            // SAFETY: terminated process handle stays valid and output is writable.
            if unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Some(code))
        }
        _ => Err(io::Error::last_os_error()),
    }
}

fn system_directories() -> io::Result<(PathBuf, PathBuf)> {
    let mut system = [0u16; 32_768];
    let mut windows = [0u16; 32_768];
    // SAFETY: writable UTF-16 buffers with exact code-unit counts; Windows
    // returns trusted installation paths rather than relying on environment data.
    let a = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    // SAFETY: same API output-buffer contract as above.
    let b = unsafe { GetWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) } as usize;
    if a == 0 || b == 0 {
        return Err(io::Error::last_os_error());
    }
    if a >= system.len() || b >= windows.len() {
        return Err(invalid("Windows installation path length"));
    }
    Ok((
        PathBuf::from(OsString::from_wide(&system[..a])),
        PathBuf::from(OsString::from_wide(&windows[..b])),
    ))
}

fn run_icacls(system_dir: &Path, path: &Path, operation: &str, entry: &str) -> io::Result<()> {
    if !matches!(operation, "/grant" | "/remove:g" | "/setintegritylevel") {
        return Err(invalid("ACL operation"));
    }
    let mut command = Command::new(system_dir.join("icacls.exe"));
    command
        .env_clear()
        .arg(path)
        .arg(operation)
        .arg(entry)
        .arg("/Q")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    if let Some(windows) = system_dir.parent() {
        command.env("SystemRoot", windows);
    }
    let mut child = command.spawn()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other("native sandbox ACL operation failed"))
                };
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "native sandbox ACL operation timed out",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wide(text: &OsStr) -> io::Result<Vec<u16>> {
    let mut result: Vec<u16> = text.encode_wide().collect();
    if result.len() > 32_766 || result.contains(&0) {
        return Err(invalid("native UTF-16 input"));
    }
    result.push(0);
    Ok(result)
}

fn command_line(executable: &Path, args: &[String]) -> io::Result<Vec<u16>> {
    let executable = executable
        .to_str()
        .ok_or_else(|| invalid("native executable Unicode"))?;
    let mut text = quote_arg(executable);
    for argument in args {
        text.push(' ');
        text.push_str(&quote_arg(argument));
    }
    wide(OsStr::new(&text))
}

fn quote_arg(text: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0usize;
    for ch in text.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
            out.push('"');
        } else {
            out.extend(std::iter::repeat_n('\\', slashes));
            out.push(ch);
        }
        slashes = 0;
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}

fn environment_block(
    windows_dir: &Path,
    system_dir: &Path,
    scratch: &Path,
) -> io::Result<Vec<u16>> {
    let mut entries = std::collections::BTreeMap::new();
    entries.insert("PATH", system_dir);
    entries.insert("SystemRoot", windows_dir);
    entries.insert("TEMP", scratch);
    entries.insert("TMP", scratch);
    let mut block = Vec::new();
    for (key, path) in entries {
        let text = format!(
            "{key}={}",
            path.to_str()
                .ok_or_else(|| invalid("native environment Unicode"))?
        );
        let value = wide(OsStr::new(&text))?;
        block.extend(value);
    }
    block.push(0);
    Ok(block)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn storage_bytes(roots: &[PathBuf]) -> io::Result<u64> {
    scan_storage(roots, 256, 8, MAX_WORKER_STORAGE_BYTES)
}

fn scan_storage(
    roots: &[PathBuf],
    max_entries: usize,
    max_depth: usize,
    max_bytes: u64,
) -> io::Result<u64> {
    let mut pending: Vec<(PathBuf, usize)> = roots.iter().cloned().map(|path| (path, 0)).collect();
    let mut scanned = 0usize;
    let mut bytes = 0u64;
    while let Some((path, depth)) = pending.pop() {
        if depth > max_depth || path.as_os_str().len() > 4096 {
            return Err(io::Error::other("native storage traversal bound"));
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        scanned += 1;
        if scanned > max_entries || metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other("native storage entry/reparse bound"));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(&path)? {
                if pending.len() + scanned >= max_entries {
                    return Err(io::Error::other("native storage entry bound"));
                }
                pending.push((entry?.path(), depth + 1));
            }
        } else if metadata.is_file() {
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or_else(|| io::Error::other("native storage size overflow"))?;
            if bytes > max_bytes {
                return Err(io::Error::other("native storage byte bound"));
            }
        } else {
            return Err(io::Error::other("native storage file type"));
        }
    }
    Ok(bytes)
}

fn windows_path_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    text.strip_prefix("\\\\?\\")
        .unwrap_or(&text)
        .trim_end_matches('\\')
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_quoting_preserves_literal_backslashes_quotes_and_empty_arguments() {
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg("C:\\path\\"), "\"C:\\path\\\\\"");
        assert_eq!(quote_arg("a\"b"), "\"a\\\"b\"");
    }
    #[test]
    fn profile_names_and_environment_are_narrow() {
        assert!(valid_profile_name(&format!(
            "{PROFILE_PREFIX}{}",
            "a".repeat(32)
        )));
        assert!(!valid_profile_name("NoSleepClub.NORA.Audio"));
        assert!(!valid_profile_name(
            "Other.App.01234567890123456789012345678901"
        ));
        let env = environment_block(
            Path::new("C:/Windows"),
            Path::new("C:/Windows/System32"),
            Path::new("D:/scratch"),
        )
        .unwrap();
        let text = String::from_utf16(&env).unwrap();
        assert!(!text.contains("KEY") && !text.contains("USERPROFILE") && !text.contains("PROXY"));
        assert_eq!(text.split('\0').filter(|s| !s.is_empty()).count(), 4);
        assert!(env.ends_with(&[0, 0]));
    }
    #[test]
    fn ordinary_test_host_is_not_an_app_container() {
        assert!(!is_app_container().unwrap());
    }

    #[test]
    fn storage_meter_counts_nested_scratch_and_profile_and_rejects_byte_overflow() {
        let temp = tempfile::tempdir().unwrap();
        let scratch = temp.path().join("scratch");
        let profile = temp.path().join("profile");
        fs::create_dir_all(scratch.join("nested")).unwrap();
        fs::create_dir(&profile).unwrap();
        fs::write(scratch.join("nested/audio.pcm"), [1u8; 17]).unwrap();
        fs::write(profile.join("package-data"), [2u8; 11]).unwrap();
        assert_eq!(storage_bytes(&[scratch.clone(), profile]).unwrap(), 28);
        let oversized = File::create(scratch.join("oversized.pcm")).unwrap();
        oversized.set_len(MAX_WORKER_STORAGE_BYTES + 1).unwrap();
        assert!(storage_bytes(&[scratch]).is_err());
    }

    #[test]
    fn storage_meter_rejects_unbounded_entry_count_and_depth() {
        let temp = tempfile::tempdir().unwrap();
        for n in 0..256 {
            File::create(temp.path().join(format!("entry-{n}"))).unwrap();
        }
        assert!(storage_bytes(&[temp.path().to_path_buf()]).is_err());
        let nested = tempfile::tempdir().unwrap();
        let path = (0..9).fold(nested.path().to_path_buf(), |path, _| path.join("nested"));
        fs::create_dir_all(&path).unwrap();
        assert!(storage_bytes(&[nested.path().to_path_buf()]).is_err());
    }

    #[test]
    fn storage_and_cleanup_reject_junction_before_mutating_any_acl() {
        let temp = tempfile::tempdir().unwrap();
        let scratch = temp.path().join("scratch");
        let outside = temp.path().join("outside");
        fs::create_dir(&scratch).unwrap();
        fs::create_dir(&outside).unwrap();
        let sentinel = outside.join("sentinel.txt");
        fs::write(&sentinel, "synthetic sentinel").unwrap();
        let (system, windows) = system_directories().unwrap();
        let profile_name = format!("{PROFILE_PREFIX}{}", "a".repeat(32));
        let name = wide(OsStr::new(&profile_name)).unwrap();
        let mut raw = ptr::null_mut();
        // SAFETY: deriving a SID is a read-only calculation from a bounded name;
        // no AppContainer profile is created by this unit test.
        assert!(unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut raw) } >= 0);
        let sid = Sid { raw };
        let sid = sid_string(&sid).unwrap();
        run_icacls(&system, &sentinel, "/grant", &format!("*{sid}:(R)")).unwrap();
        let acl = |path: &Path| {
            Command::new(system.join("icacls.exe"))
                .env_clear()
                .env("SystemRoot", &windows)
                .arg(path)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .unwrap()
                .stdout
        };
        let before = acl(&sentinel);
        let status=Command::new(system.join("WindowsPowerShell/v1.0/powershell.exe"))
            .env_clear().env("SystemRoot",&windows).env("NORA_TEST_LINK",scratch.join("link")).env("NORA_TEST_TARGET",&outside)
            .args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:NORA_TEST_LINK -Target $env:NORA_TEST_TARGET | Out-Null"])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW).status().unwrap();
        assert!(status.success());
        assert!(storage_bytes(std::slice::from_ref(&scratch)).is_err());
        let metadata = SandboxLease {
            profile_name,
            sid,
            acl_paths: vec![scratch.clone()],
            scratch_dir: scratch,
            profile_folder: None,
            parent_pid: std::process::id(),
            parent_creation_filetime: crate::process_creation_filetime(std::process::id()).unwrap(),
            created_unix_ms: 0,
        };
        assert!(cleanup_lease_with_system(&metadata, &system).is_err());
        assert_eq!(acl(&sentinel), before);
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "synthetic sentinel");
    }
}
