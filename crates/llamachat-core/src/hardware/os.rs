//! Operating-system identification. Name/version come from `sysinfo`; the CPU
//! architecture comes straight from the compiled target so it's always present.

use crate::types::Os;
use sysinfo::System;

pub fn detect() -> Os {
    let name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
    // Prefer the numeric OS version; fall back to the kernel version so the
    // field is never empty (important for the profile()'s non-empty invariant).
    let version = System::os_version()
        .or_else(System::kernel_version)
        .unwrap_or_default();

    Os {
        name,
        version,
        arch: std::env::consts::ARCH.to_string(),
    }
}
