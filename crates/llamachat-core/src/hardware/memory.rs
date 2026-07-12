//! System RAM detection via `sysinfo`. Values are reported in mebibytes (MiB).

use crate::types::Memory;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

const BYTES_PER_MB: u64 = 1024 * 1024;

pub fn detect() -> Memory {
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
    );
    Memory {
        total_mb: sys.total_memory() / BYTES_PER_MB,
        available_mb: sys.available_memory() / BYTES_PER_MB,
    }
}
