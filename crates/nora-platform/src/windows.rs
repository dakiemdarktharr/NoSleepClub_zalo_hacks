use crate::{JobMemory, ProcessMemory, validate_memory_limit};
use std::{
    io,
    mem::{size_of, zeroed},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
    sync::{Arc, OnceLock},
};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, FILETIME, GetLastError, SetLastError},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
            JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicProcessIdList,
            JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        },
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
        RemoteDesktop::{
            WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTSFreeMemory, WTSINFOEXW,
            WTSQuerySessionInformationW, WTSSessionInfoEx,
        },
        Threading::{
            CreateMutexW, GetCurrentProcess, GetProcessTimes, OpenProcess,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
        },
    },
};

/// One normal assistant per interactive Windows session, independent of user
/// configuration paths. The presence of a held named object is the ownership.
pub struct SingleInstance {
    _handle: OwnedHandle,
}
impl SingleInstance {
    pub fn acquire() -> io::Result<Self> {
        let name: Vec<u16> = "Local\\NoSleepClub.NORA.Desktop"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: fixed NUL-terminated name, null attributes, no mutex ownership
        // to release. CreateMutex atomically distinguishes a preexisting object.
        let (raw, error) = unsafe {
            SetLastError(0);
            let raw = CreateMutexW(ptr::null(), 0, name.as_ptr());
            (raw, GetLastError())
        };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: newly returned owned handle, transferred exactly once.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        if error == ERROR_ALREADY_EXISTS {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "NORA is already running in this Windows session",
            ));
        }
        Ok(Self { _handle: handle })
    }
}

/// A process-tree commit cap installed before native models or the UI start.
/// NORA-owned descendants inherit membership. Only a trusted, separately
/// approved external-user-app launcher may request breakaway. Native workers
/// additionally enter a nested job that prohibits breakaway/child processes.
/// No filesystem/network confinement follows from this resource object alone.
pub struct BudgetJob {
    handle: OwnedHandle,
    limit: u64,
}

impl BudgetJob {
    pub fn install(bytes: u64) -> io::Result<Self> {
        let limit = validate_memory_limit(bytes)?;
        Self::install_limit(bytes, limit, false)
    }

    /// Fixed LifeMate scoring profile; does not relax the desktop constructor
    /// or its validation. This does not create 64 GB of physical host memory.
    pub fn install_contest() -> io::Result<Arc<Self>> {
        // A job containing this process must not close its last handle during
        // normal Rust teardown: KILL_ON_JOB_CLOSE would kill this process too.
        // This single non-inheritable handle lives until OS process teardown,
        // including abrupt termination, then Windows kills all descendants.
        // OnceLock also prevents racing installations of multiple root jobs.
        static CONTEST_JOB: OnceLock<io::Result<Arc<BudgetJob>>> = OnceLock::new();
        match CONTEST_JOB.get_or_init(|| {
            let bytes = crate::CONTEST_RAM_LIMIT_BYTES;
            let limit = usize::try_from(bytes)
                .map_err(|_| io::Error::other("contest profile needs a 64-bit target"))?;
            Self::install_limit(bytes, limit, true).map(Arc::new)
        }) {
            Ok(job) => Ok(Arc::clone(job)),
            Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
        }
    }

    fn install_limit(bytes: u64, limit: usize, contest: bool) -> io::Result<Self> {
        // SAFETY: null security/name request an unnamed, non-inheritable job;
        // success returns a newly owned kernel handle, transferred exactly once.
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: raw was returned by successful CreateJobObjectW and is owned.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        // SAFETY: Win32's POD info struct permits the all-zero initial state.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_JOB_MEMORY
            | if contest {
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            } else {
                JOB_OBJECT_LIMIT_BREAKAWAY_OK
            };
        info.JobMemoryLimit = limit;
        // SAFETY: valid job handle, matching info class/struct/byte size; the
        // function borrows this initialized buffer only for the duration of call.
        if unsafe {
            SetInformationJobObject(
                handle.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: current-process pseudo handle is valid and not closed by us;
        // job remains alive in the guard. Failure is fatal rather than uncapped.
        if unsafe { AssignProcessToJobObject(handle.as_raw_handle(), GetCurrentProcess()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            handle,
            limit: bytes,
        })
    }

    pub fn snapshot(&self) -> io::Result<JobMemory> {
        // SAFETY: POD output struct is zero-initialized and passed with exact size.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.handle.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&mut info as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // Native layout: two DWORDs followed by pointer-sized process IDs.
        // usize storage supplies the alignment and enough capacity for 128 IDs.
        let mut ids = [0usize; 130];
        if unsafe {
            QueryInformationJobObject(
                self.handle.as_raw_handle(),
                JobObjectBasicProcessIdList,
                ids.as_mut_ptr().cast(),
                size_of::<[usize; 130]>() as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the API initialized two DWORD header values in a properly
        // aligned buffer, and the count is range-checked before reading IDs.
        let count = unsafe { *(ids.as_ptr().cast::<u32>().add(1)) } as usize;
        if count > 128 {
            return Err(io::Error::other("process tree exceeds meter capacity"));
        }
        let offset = 8 / size_of::<usize>();
        let process_ids = ids[offset..offset + count]
            .iter()
            .map(|id| u32::try_from(*id).map_err(|_| io::Error::other("invalid process id")))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(JobMemory {
            limit_bytes: self.limit,
            peak_job_commit_bytes: info.PeakJobMemoryUsed as u64,
            process_ids,
        })
    }
}

pub fn process_memory(pid: u32) -> io::Result<ProcessMemory> {
    // SAFETY: process ID is numeric; no handle inheritance requested. We only
    // query counters, never write the process. All returned handles are closed.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcess returned a new owned handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    // SAFETY: native output struct permits zero initialization.
    let mut info: PROCESS_MEMORY_COUNTERS_EX = unsafe { zeroed() };
    info.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: extended counters begin with the standard struct, and cb specifies
    // the full allocated output size as required by GetProcessMemoryInfo.
    if unsafe {
        GetProcessMemoryInfo(
            handle.as_raw_handle(),
            (&mut info as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            info.cb,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(ProcessMemory {
        pid,
        working_set_bytes: info.WorkingSetSize as u64,
        peak_working_set_bytes: info.PeakWorkingSetSize as u64,
        private_commit_bytes: info.PrivateUsage as u64,
    })
}

pub fn process_creation_filetime(pid: u32) -> io::Result<u64> {
    // SAFETY: numeric PID and a query-only, non-inheritable process handle.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcess transfers one owned handle to this scope.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    // SAFETY: handle is live and four outputs are valid initialized FILETIMEs.
    if unsafe {
        GetProcessTimes(
            handle.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

pub fn session_is_locked() -> io::Result<bool> {
    let mut buffer: *mut u16 = ptr::null_mut();
    let mut bytes = 0u32;
    // SAFETY: current session/server constants select this user's own session;
    // API allocates the returned buffer and we release it on every success path.
    if unsafe {
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if buffer.is_null() || bytes < size_of::<WTSINFOEXW>() as u32 {
        if !buffer.is_null() {
            unsafe { WTSFreeMemory(buffer.cast()) };
        }
        return Err(io::Error::other("invalid Windows session state"));
    }
    // SAFETY: byte size checked above; WTSInfoEx returns this struct, not a string.
    let result = unsafe {
        let info = &*buffer.cast::<WTSINFOEXW>();
        if info.Level != 1 {
            Err(io::Error::other("unsupported session state level"))
        } else {
            match info.Data.WTSInfoExLevel1.SessionFlags {
                0 => Ok(true),
                1 => Ok(false),
                _ => Err(io::Error::other("unknown session lock state")),
            }
        }
    };
    // SAFETY: buffer was allocated by WTSQuerySessionInformationW.
    unsafe { WTSFreeMemory(buffer.cast()) };
    result
}
