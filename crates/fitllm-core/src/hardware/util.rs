//! Small cross-platform helpers shared by the hardware detection submodules.
//! Everything here is strictly read-only and never panics.

use std::path::PathBuf;
use std::process::Command;

/// Resolve the current user's home directory without pulling in an extra crate.
///
/// Uses `HOME` on Unix and `USERPROFILE` (falling back to `HOMEDRIVE`+`HOMEPATH`)
/// on Windows. Returns `None` if nothing usable is set.
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(p) = std::env::var_os("USERPROFILE") {
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
        if let (Some(drive), Some(path)) =
            (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
        {
            let mut s = drive;
            s.push(path);
            return Some(PathBuf::from(s));
        }
        None
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    }
}

/// Return true if `name` is an executable found on `PATH`. This lets us probe
/// for optional tools (`vulkaninfo`, `rocm-smi`, …) without actually spawning
/// them, which keeps profiling fast and avoids noisy/slow tools.
pub fn command_exists(name: &str) -> bool {
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    // On Windows an executable may carry one of the PATHEXT extensions.
    #[cfg(windows)]
    let exts: Vec<String> = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".EXE;.BAT;.CMD;.COM".into())
        .split(';')
        .map(|s| s.to_ascii_lowercase())
        .collect();

    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return true;
        }
        #[cfg(windows)]
        for ext in &exts {
            if dir.join(format!("{name}{ext}")).is_file() {
                return true;
            }
        }
    }
    false
}

/// Run a command and return its trimmed stdout on success, or `None` if the
/// binary is missing, exits non-zero, or produces no output. Never panics.
pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
