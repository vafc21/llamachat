//! Apple Silicon detection. Only meaningful on macOS + aarch64, where the SoC
//! uses unified memory (shared CPU/GPU RAM), which changes the model-size math
//! versus discrete VRAM. Returns `None` on every other platform.
//!
//! This path cannot be exercised on the Linux build host, so it is written from
//! the documented `sysctl` / `system_profiler` behaviour and degrades to sane
//! defaults if a probe fails.

#[allow(unused_imports)]
use super::util;
use crate::types::AppleSilicon;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub fn detect() -> Option<AppleSilicon> {
    // Chip name, e.g. "Apple M2 Pro". `machdep.cpu.brand_string` is the most
    // reliable source across macOS versions.
    let chip = util::run("sysctl", &["-n", "machdep.cpu.brand_string"])
        .unwrap_or_else(|| "Apple Silicon".to_string());

    // GPU core count from `system_profiler`. The line reads "Total Number of
    // Cores: N" under SPDisplaysDataType. Parsed leniently.
    let gpu_cores = util::run("system_profiler", &["SPDisplaysDataType"])
        .and_then(|out| {
            out.lines()
                .find(|l| l.contains("Total Number of Cores"))
                .and_then(|l| l.split(':').nth(1))
                .and_then(|v| v.trim().split_whitespace().next())
                .and_then(|n| n.parse::<u32>().ok())
        });

    Some(AppleSilicon {
        unified_memory: true,           // Always true on Apple Silicon SoCs.
        gpu_cores,
        neural_engine: true,            // Every Apple Silicon chip ships an ANE.
        chip,
    })
}

/// Non-Apple-Silicon platforms have no unified-memory SoC to describe.
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub fn detect() -> Option<AppleSilicon> {
    None
}
