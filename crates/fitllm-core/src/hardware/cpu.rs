//! CPU detection: model/vendor/core counts/clocks via `sysinfo`, plus
//! instruction-set flags via the standard library's runtime feature detection
//! (no extra crate needed). Clocks on Linux come from `sysfs` cpufreq nodes.

use crate::types::{Cpu, CpuFlags};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

pub fn detect() -> Cpu {
    // Only refresh CPU info — we don't need the (slower) process/memory scan.
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    );

    let cpus = sys.cpus();
    let first = cpus.first();

    let model = first
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let vendor = first
        .map(|c| c.vendor_id().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let logical_cores = cpus.len() as u32;
    // Physical cores can be unavailable in some virtualized environments; fall
    // back to the logical count so the number is never zero.
    let physical_cores = sys
        .physical_core_count()
        .map(|n| n as u32)
        .filter(|&n| n > 0)
        .unwrap_or(logical_cores);

    let (base_clock_mhz, max_clock_mhz) = clocks(first.map(|c| c.frequency()));

    Cpu {
        model,
        vendor,
        physical_cores,
        logical_cores,
        base_clock_mhz,
        max_clock_mhz,
        flags: detect_flags(),
    }
}

/// Best-effort base/max clock in MHz.
///
/// * Linux: read the kernel-reported cpufreq nodes in sysfs. `cpuinfo_max_freq`
///   is the hardware max; `base_frequency` (Intel only) is the nominal base.
/// * Other platforms: sysfs isn't available, so we fall back to `sysinfo`'s
///   current frequency as the `max` approximation and leave `base` unknown.
fn clocks(sysinfo_current_mhz: Option<u64>) -> (Option<f64>, Option<f64>) {
    #[cfg(target_os = "linux")]
    {
        let read_khz = |path: &str| -> Option<f64> {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .map(|khz| khz / 1000.0) // kHz -> MHz
        };
        let base = read_khz("/sys/devices/system/cpu/cpu0/cpufreq/base_frequency");
        let max = read_khz("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
            .or_else(|| sysinfo_current_mhz.filter(|&f| f > 0).map(|f| f as f64));
        return (base, max);
    }
    #[cfg(not(target_os = "linux"))]
    {
        // `sysinfo` only exposes the *current* frequency; use it as the best
        // available "max" estimate and leave base clock unknown.
        let max = sysinfo_current_mhz.filter(|&f| f > 0).map(|f| f as f64);
        (None, max)
    }
}

/// Detect the instruction-set features that matter for LLM inference.
///
/// x86/x86_64 features come from the standard library's `is_x86_feature_detected!`
/// (a CPUID probe, works on Linux/macOS/Windows). On aarch64, NEON is mandatory
/// in the ARMv8-A baseline, so it is always present.
fn detect_flags() -> CpuFlags {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        CpuFlags {
            avx2: std::is_x86_feature_detected!("avx2"),
            avx512: std::is_x86_feature_detected!("avx512f"),
            fma: std::is_x86_feature_detected!("fma"),
            f16c: std::is_x86_feature_detected!("f16c"),
            neon: false,
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        CpuFlags {
            avx2: false,
            avx512: false,
            fma: false,
            f16c: false,
            neon: true, // NEON is part of the mandatory ARMv8-A baseline.
        }
    }
    #[cfg(not(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "aarch64"
    )))]
    {
        CpuFlags::default()
    }
}
