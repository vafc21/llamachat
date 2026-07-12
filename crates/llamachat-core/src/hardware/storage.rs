//! Storage detection: where LlamaChat keeps (or would keep) its model blobs, and
//! how much room is left on that volume. A sequential read benchmark is
//! intentionally skipped to keep profiling fast and strictly non-destructive
//! (see `read_mbps` below).

use super::util;
use crate::types::Storage;
use std::path::{Path, PathBuf};
use sysinfo::Disks;

const BYTES_PER_MB: u64 = 1024 * 1024;

pub fn detect() -> Storage {
    let dir = models_dir();
    let free_mb = free_mb_for(&dir);
    Storage {
        models_dir: dir.to_string_lossy().to_string(),
        free_mb,
        // Left as `None` on purpose: an honest sequential-read measurement means
        // reading real files off disk, which is slow and could touch large model
        // blobs. The recommendation engine treats this as "unknown" and the
        // Python sidecar can measure it during a benchmark run instead.
        read_mbps: None,
    }
}

/// Resolve the directory LlamaChat uses for model storage.
///
/// Order of preference:
/// 1. `FITLLM_MODELS_DIR` env var (explicit user override).
/// 2. `<home>/.cache/llamachat/models` — the default on Linux/macOS. (On Windows
///    this still lands under the user profile via `home_dir()`.)
/// 3. A platform-sensible fallback if the home directory can't be resolved.
fn models_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("FITLLM_MODELS_DIR").filter(|s| !s.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(home) = util::home_dir() {
        return home.join(".cache").join("llamachat").join("models");
    }
    // Last-resort fallbacks when even $HOME is unset.
    // TODO(windows): prefer %LOCALAPPDATA%\llamachat\models once we resolve it.
    #[cfg(windows)]
    {
        PathBuf::from(r"C:\Users\Default\.cache\llamachat\models")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/root/.cache/llamachat/models")
    }
}

/// Free space (MiB) on the filesystem that holds `dir`.
///
/// We pick the mounted disk whose mount point is the longest prefix of `dir`
/// (so `/home` wins over `/` for a path under `/home`). This works even when
/// `dir` doesn't exist yet, since we only compare paths. Falls back to `0` if
/// no disks are reported.
fn free_mb_for(dir: &Path) -> u64 {
    let disks = Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64)> = None; // (mount depth, available bytes)
    for disk in disks.list() {
        let mount = disk.mount_point();
        if dir.starts_with(mount) {
            let depth = mount.components().count();
            let better = best.map_or(true, |(d, _)| depth > d);
            if better {
                best = Some((depth, disk.available_space()));
            }
        }
    }
    best.map(|(_, bytes)| bytes / BYTES_PER_MB).unwrap_or(0)
}
