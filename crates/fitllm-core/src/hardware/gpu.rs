//! GPU detection. NVIDIA (via `nvidia-smi`) is the primary, fully-tested path on
//! this build host. AMD (`rocm-smi`) and Intel iGPU (`sysfs`) detection is
//! best-effort and cannot be exercised here, so it degrades gracefully to an
//! empty result rather than guessing. All probes are read-only.

use super::util;
use crate::types::Gpu;

/// Detect every discrete/integrated GPU we can find. Returns an empty vec when
/// no GPU tooling is present (a CPU-only machine), never an error.
pub fn detect() -> Vec<Gpu> {
    let mut gpus = Vec::new();
    gpus.extend(detect_nvidia());
    gpus.extend(detect_amd());
    #[cfg(target_os = "linux")]
    gpus.extend(detect_intel_igpu());
    gpus
}

// ---------------------------------------------------------------------------
// NVIDIA (CUDA)
// ---------------------------------------------------------------------------

/// Query NVIDIA GPUs with `nvidia-smi`. One line per GPU:
/// `name, memory.total, memory.free, driver_version, compute_cap`
/// (`--format=csv,noheader,nounits` so every field is a bare value in MiB/text).
fn detect_nvidia() -> Vec<Gpu> {
    let out = match util::run(
        "nvidia-smi",
        &[
            "--query-gpu=name,memory.total,memory.free,driver_version,compute_cap",
            "--format=csv,noheader,nounits",
        ],
    ) {
        Some(o) => o,
        None => return Vec::new(),
    };

    // The CUDA toolkit version lives in the plain `nvidia-smi` banner, not the
    // query output, so fetch it once and share it across all GPUs.
    let cuda_version = nvidia_cuda_version();

    let mut gpus = Vec::new();
    for line in out.lines() {
        let fields: Vec<&str> = line.split(',').map(|f| f.trim()).collect();
        if fields.is_empty() || fields[0].is_empty() {
            continue;
        }
        let field = |i: usize| fields.get(i).map(|s| s.to_string()).filter(|s| !s.is_empty());
        let mb = |i: usize| field(i).and_then(|s| s.parse::<u64>().ok());

        gpus.push(Gpu {
            vendor: "NVIDIA".to_string(),
            model: field(0).unwrap_or_else(|| "NVIDIA GPU".to_string()),
            vram_total_mb: mb(1),
            vram_free_mb: mb(2),
            driver_version: field(3),
            cuda_version: cuda_version.clone(),
            compute_capability: field(4),
            backend: "cuda".to_string(),
            is_integrated: false,
        });
    }
    gpus
}

/// Parse the "CUDA Version: X.Y" field from the `nvidia-smi` banner.
fn nvidia_cuda_version() -> Option<String> {
    let out = util::run("nvidia-smi", &[])?;
    let marker = "CUDA Version:";
    let idx = out.find(marker)? + marker.len();
    let ver: String = out[idx..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if ver.is_empty() {
        None
    } else {
        Some(ver)
    }
}

// ---------------------------------------------------------------------------
// AMD (ROCm)
// ---------------------------------------------------------------------------

/// Best-effort AMD detection via `rocm-smi`. Untested on this box; kept
/// deliberately conservative — if the JSON shape isn't what we expect we still
/// emit a GPU entry with whatever we could read rather than crashing.
fn detect_amd() -> Vec<Gpu> {
    // `--showproductname` lists each card; combine with vram info. We ask for
    // JSON so parsing is stable across rocm-smi versions.
    let out = match util::run(
        "rocm-smi",
        &["--showproductname", "--showmeminfo", "vram", "--json"],
    ) {
        Some(o) => o,
        None => return Vec::new(),
    };

    let json: serde_json::Value = match serde_json::from_str(&out) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let driver = util::run("rocm-smi", &["--showdriverversion", "--json"])
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| find_first_string(&v, "Driver version"));

    let mut gpus = Vec::new();
    if let Some(obj) = json.as_object() {
        // Top-level keys look like "card0", "card1", … Skip the "system" key.
        for (card, info) in obj {
            if !card.starts_with("card") {
                continue;
            }
            let model = find_first_string(info, "Card Series")
                .or_else(|| find_first_string(info, "Card model"))
                .or_else(|| find_first_string(info, "Card SKU"))
                .unwrap_or_else(|| "AMD GPU".to_string());
            // VRAM total is reported in bytes under a "VRAM Total Memory (B)" key.
            let vram_total_mb = find_first_string(info, "VRAM Total Memory (B)")
                .and_then(|s| s.parse::<u64>().ok())
                .map(|b| b / (1024 * 1024));
            let vram_free_mb = find_first_string(info, "VRAM Total Used Memory (B)")
                .and_then(|s| s.parse::<u64>().ok())
                .and_then(|used| vram_total_mb.map(|tot| tot.saturating_sub(used / (1024 * 1024))));

            gpus.push(Gpu {
                vendor: "AMD".to_string(),
                model,
                vram_total_mb,
                vram_free_mb,
                driver_version: driver.clone(),
                cuda_version: None,
                compute_capability: None,
                backend: "rocm".to_string(),
                is_integrated: false,
            });
        }
    }
    gpus
}

/// Recursively search a JSON value for the first string value stored under a
/// key that contains `needle` (case-insensitive). Helps absorb rocm-smi's
/// slightly different key spellings across versions.
fn find_first_string(value: &serde_json::Value, needle: &str) -> Option<String> {
    let needle = needle.to_ascii_lowercase();
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if k.to_ascii_lowercase().contains(&needle) {
                    if let Some(s) = v.as_str() {
                        return Some(s.to_string());
                    }
                }
                if let Some(found) = find_first_string(v, &needle) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Intel integrated graphics (Linux sysfs)
// ---------------------------------------------------------------------------

/// Detect an Intel iGPU by scanning `/sys/class/drm` for a card whose PCI vendor
/// id is Intel (0x8086). Integrated GPUs share system RAM, so `vram_*` is left
/// unknown. Best-effort and Linux-only; skipped entirely elsewhere.
#[cfg(target_os = "linux")]
fn detect_intel_igpu() -> Vec<Gpu> {
    let mut gpus = Vec::new();
    let entries = match std::fs::read_dir("/sys/class/drm") {
        Ok(e) => e,
        Err(_) => return gpus,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Only real render nodes named card0/card1/…, not the card0-HDMI-A-1
        // connector sub-nodes.
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let vendor_path = entry.path().join("device/vendor");
        let vendor = std::fs::read_to_string(&vendor_path)
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if vendor != "0x8086" {
            continue;
        }
        gpus.push(Gpu {
            vendor: "Intel".to_string(),
            model: "Intel integrated GPU".to_string(),
            vram_total_mb: None,
            vram_free_mb: None,
            driver_version: None,
            cuda_version: None,
            compute_capability: None,
            // Vulkan is the practical inference backend for Intel iGPUs.
            backend: "vulkan".to_string(),
            is_integrated: true,
        });
    }
    gpus
}
