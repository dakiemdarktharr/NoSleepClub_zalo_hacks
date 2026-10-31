//! Minimal Windows-only FFI boundary. Resource limits are not a filesystem or
//! network sandbox. Native worker isolation is configured separately.

use serde::{Deserialize, Serialize};
use std::io;

pub const RAM_LIMIT_BYTES: u64 = 2_000_000_000;
pub const VRAM_LIMIT_BYTES: u64 = 4_000_000_000;
pub const DISK_LIMIT_BYTES: u64 = 15_000_000_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessMemory {
    pub pid: u32,
    pub working_set_bytes: u64,
    pub peak_working_set_bytes: u64,
    pub private_commit_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobMemory {
    pub limit_bytes: u64,
    pub peak_job_commit_bytes: u64,
    pub process_ids: Vec<u32>,
}

#[cfg(windows)]
mod measurement;
#[cfg(windows)]
pub mod sandbox;
#[cfg(windows)]
pub use measurement::{Measurement, MemoryMeasurement};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{
    BudgetJob, SingleInstance, process_creation_filetime, process_memory, session_is_locked,
};

#[cfg(not(windows))]
pub fn process_memory(_pid: u32) -> io::Result<ProcessMemory> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows process counters are required",
    ))
}

#[cfg(not(windows))]
pub fn session_is_locked() -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows session state is required",
    ))
}

pub fn validate_memory_limit(bytes: u64) -> io::Result<usize> {
    if !(32_000_000..=RAM_LIMIT_BYTES).contains(&bytes) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "process-tree memory limit is out of range",
        ));
    }
    bytes.try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "memory limit does not fit this architecture",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_cap_cannot_be_disabled_or_increased_by_config() {
        assert!(validate_memory_limit(RAM_LIMIT_BYTES).is_ok());
        assert!(validate_memory_limit(RAM_LIMIT_BYTES + 1).is_err());
        assert!(validate_memory_limit(0).is_err());
    }
}
