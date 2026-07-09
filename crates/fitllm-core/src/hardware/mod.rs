//! Cross-platform, read-only hardware profiler for FitLLM.
//!
//! [`profile`] gathers a normalized snapshot of the machine — CPU, GPU(s),
//! Apple Silicon SoC info, RAM, storage and OS — plus the list of acceleration
//! backends available for LLM inference. Everything here is strictly read-only
//! and non-destructive: we read `sysfs`/`/proc`, call `sysinfo`, and shell out
//! to vendor tools (`nvidia-smi`, `rocm-smi`) with query-only flags. No probe
//! is allowed to panic — each submodule degrades to `None`/empty on failure.
//!
//! The Linux + NVIDIA path is the fully-tested one on the current build host;
//! macOS and Windows paths are gated with `cfg` and written from documented
//! tool behaviour (see the per-module comments) but are not exercised here.

mod apple;
mod cpu;
mod gpu;
mod memory;
mod os;
mod storage;
mod util;

use crate::types::{AppleSilicon, Gpu, HardwareProfile};
use anyhow::Result;

/// Detect the current machine's hardware profile.
///
/// Returns `Ok` on every real machine; the individual detectors never fail, so
/// the `Result` exists mainly to satisfy the shared contract and leave room for
/// future fallible steps. Completes in well under a second on typical hardware.
pub fn profile() -> Result<HardwareProfile> {
    let cpu = cpu::detect();
    let gpus = gpu::detect();
    let apple_silicon = apple::detect();
    let memory = memory::detect();
    let storage = storage::detect();
    let os = os::detect();
    let backends = detect_backends(&gpus, &apple_silicon);

    Ok(HardwareProfile {
        cpu,
        gpus,
        apple_silicon,
        memory,
        storage,
        os,
        backends,
        detected_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Build the ordered list of acceleration backends available on this machine.
///
/// Hardware accelerators come first (best-first), with `"cpu"` always present
/// as the universal fallback. Examples: `["cuda", "cpu"]` on this NVIDIA box,
/// `["metal", "cpu"]` on Apple Silicon.
fn detect_backends(gpus: &[Gpu], apple_silicon: &Option<AppleSilicon>) -> Vec<String> {
    let mut backends: Vec<String> = Vec::new();
    let push = |b: &str, list: &mut Vec<String>| {
        if !list.iter().any(|x| x == b) {
            list.push(b.to_string());
        }
    };

    // Derive accelerator backends from the GPUs we actually detected.
    for gpu in gpus {
        match gpu.backend.as_str() {
            "cuda" => push("cuda", &mut backends),
            "rocm" => push("rocm", &mut backends),
            _ => {}
        }
    }

    // Metal is available on any Apple Silicon Mac even before we enumerate GPUs.
    if apple_silicon.is_some() {
        push("metal", &mut backends);
    }

    // Vulkan is a viable backend if the runtime/loader is installed.
    if util::command_exists("vulkaninfo") {
        push("vulkan", &mut backends);
    }

    // CPU inference is always possible.
    push("cpu", &mut backends);
    backends
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_succeeds_with_basic_invariants() {
        let p = profile().expect("hardware profile should always succeed");

        // At least one logical CPU core must be reported.
        assert!(
            p.cpu.logical_cores >= 1,
            "expected >= 1 logical core, got {}",
            p.cpu.logical_cores
        );

        // The OS name must be non-empty.
        assert!(!p.os.name.is_empty(), "os.name should not be empty");

        // The backend list always includes a CPU fallback, and a timestamp is set.
        assert!(
            p.backends.iter().any(|b| b == "cpu"),
            "backends should include cpu, got {:?}",
            p.backends
        );
        assert!(!p.detected_at.is_empty(), "detected_at should be set");
    }
}
