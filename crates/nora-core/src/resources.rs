//! Checked resource admission for owned buffers, model residency and workers.
//!
//! Reservations are estimates maintained by trusted runtime code. This is not an
//! RSS/VRAM meter or an OS limit. Count the complete process tree separately;
//! claiming a small reservation does not prove the actual program fits 2 GB.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CoreError, MAX_ID_BYTES, nonempty};

pub const MAX_RESERVATIONS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    pub ram_bytes: u64,
    pub vram_bytes: u64,
    pub disk_bytes: u64,
    pub workers: u32,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            ram_bytes: 2_000_000_000,
            // Headroom on the user's 4 GB device. This is an admission target,
            // not a claim that any selected ASR/TTS profile already fits.
            vram_bytes: 3_000_000_000,
            disk_bytes: 15_000_000_000,
            workers: 2,
        }
    }
}

impl ResourceBudget {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.ram_bytes == 0 || self.disk_bytes == 0 || self.workers == 0 || self.workers > 64 {
            return Err(CoreError::Invalid {
                field: "budget",
                reason: "RAM/disk/workers must be positive; workers cannot exceed64",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResourceUsage {
    pub ram_bytes: u64,
    pub vram_bytes: u64,
    pub disk_bytes: u64,
    pub workers: u32,
}

impl ResourceUsage {
    fn checked_add(self, other: Self) -> Result<Self, CoreError> {
        Ok(Self {
            ram_bytes: self
                .ram_bytes
                .checked_add(other.ram_bytes)
                .ok_or(CoreError::Overflow)?,
            vram_bytes: self
                .vram_bytes
                .checked_add(other.vram_bytes)
                .ok_or(CoreError::Overflow)?,
            disk_bytes: self
                .disk_bytes
                .checked_add(other.disk_bytes)
                .ok_or(CoreError::Overflow)?,
            workers: self
                .workers
                .checked_add(other.workers)
                .ok_or(CoreError::Overflow)?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, CoreError> {
        Ok(Self {
            ram_bytes: self
                .ram_bytes
                .checked_sub(other.ram_bytes)
                .ok_or(CoreError::Overflow)?,
            vram_bytes: self
                .vram_bytes
                .checked_sub(other.vram_bytes)
                .ok_or(CoreError::Overflow)?,
            disk_bytes: self
                .disk_bytes
                .checked_sub(other.disk_bytes)
                .ok_or(CoreError::Overflow)?,
            workers: self
                .workers
                .checked_sub(other.workers)
                .ok_or(CoreError::Overflow)?,
        })
    }

    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug)]
pub struct BudgetPool {
    limits: ResourceBudget,
    usage: ResourceUsage,
    reservations: BTreeMap<String, ResourceUsage>,
}

impl BudgetPool {
    pub fn new(limits: ResourceBudget) -> Result<Self, CoreError> {
        limits.validate()?;
        Ok(Self {
            limits,
            usage: ResourceUsage::default(),
            reservations: BTreeMap::new(),
        })
    }

    pub fn limits(&self) -> ResourceBudget {
        self.limits
    }

    pub fn usage(&self) -> ResourceUsage {
        self.usage
    }

    pub fn reservation_count(&self) -> usize {
        self.reservations.len()
    }

    /// An ID must have a single runtime owner. Model instances shared by logical
    /// workers should be reserved once by the model owner, not once per role.
    pub fn try_reserve(&mut self, id: &str, request: ResourceUsage) -> Result<(), CoreError> {
        nonempty(id, "reservation_id", MAX_ID_BYTES)?;
        if request.is_empty() {
            return Err(CoreError::Invalid {
                field: "reservation",
                reason: "cannot reserve zero resources",
            });
        }
        if self.reservations.contains_key(id) {
            return Err(CoreError::DuplicateReservation);
        }
        if self.reservations.len() >= MAX_RESERVATIONS {
            return Err(CoreError::SizeLimit {
                field: "reservations",
            });
        }
        let next = self.usage.checked_add(request)?;
        if next.ram_bytes > self.limits.ram_bytes {
            return Err(CoreError::BudgetExceeded("RAM"));
        }
        if next.vram_bytes > self.limits.vram_bytes {
            return Err(CoreError::BudgetExceeded("VRAM"));
        }
        if next.disk_bytes > self.limits.disk_bytes {
            return Err(CoreError::BudgetExceeded("disk"));
        }
        if next.workers > self.limits.workers {
            return Err(CoreError::BudgetExceeded("workers"));
        }
        self.reservations.insert(id.to_owned(), request);
        self.usage = next;
        Ok(())
    }

    pub fn release(&mut self, id: &str) -> Result<ResourceUsage, CoreError> {
        let request = *self
            .reservations
            .get(id)
            .ok_or(CoreError::UnknownReservation)?;
        let next = self.usage.checked_sub(request)?;
        self.reservations.remove(id);
        self.usage = next;
        Ok(request)
    }
}
