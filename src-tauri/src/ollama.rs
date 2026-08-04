//! Ollama process + path management.
//!
//! A GUI-launched app inherits a minimal PATH that often omits Ollama's install
//! dir (macOS: not Homebrew's `/opt/homebrew/bin`; Windows: not
//! `%LOCALAPPDATA%\Programs\Ollama`), so `Command::new("ollama")` fails when the
//! app is launched from Finder / the Start Menu. We resolve the binary's
//! absolute path here, and make sure a server is actually listening before we
//! pull a model or chat — spawning `ollama serve` ourselves if it isn't.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const OLLAMA_ADDR: &str = "127.0.0.1:11434";

/// The platform's ollama executable file name.
#[cfg(target_os = "windows")]
const OLLAMA_EXE: &str = "ollama.exe";
#[cfg(not(target_os = "windows"))]
const OLLAMA_EXE: &str = "ollama";

/// Well-known install locations to probe before falling back to PATH (a
/// GUI-launched app frequently inherits a PATH that omits these).
#[cfg(target_os = "windows")]
fn wellknown_bins() -> Vec<PathBuf> {
    let mut v = Vec::new();
    // Per-user (default installer target) and per-machine locations.
    for var in ["LOCALAPPDATA", "ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Ok(base) = std::env::var(var) {
            v.push(PathBuf::from(&base).join(r"Programs\Ollama\ollama.exe"));
            v.push(PathBuf::from(&base).join(r"Ollama\ollama.exe"));
        }
    }
    v
}

#[cfg(not(target_os = "windows"))]
fn wellknown_bins() -> Vec<PathBuf> {
    [
        "/opt/homebrew/bin/ollama",
        "/usr/local/bin/ollama",
        "/usr/bin/ollama",
        "/opt/homebrew/opt/ollama/bin/ollama",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

/// Absolute path to the `ollama` binary. Searches an env override, the common
/// per-OS install locations, and finally the inherited PATH so terminal/dev
/// launches keep working too.
pub fn ollama_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FITLLM_OLLAMA_BIN") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    for pb in wellknown_bins() {
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(OLLAMA_EXE);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// True when an Ollama server is accepting connections on the default port.
/// Tags Ollama actually has locally, via `/api/tags`.
pub fn installed_tags() -> Vec<String> {
    let Ok(resp) = ureq_get(&format!("http://{OLLAMA_ADDR}/api/tags")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp) else {
        return Vec::new();
    };
    json.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Map a catalog tag onto a tag that is actually installed.
///
/// The catalog ships base tags (`qwen2.5:7b`) but people pull the variants
/// (`qwen2.5:7b-instruct-q8_0`), and Ollama does not treat those as the same
/// model — it 404s. Every catalog entry but one was unusable on a machine with
/// eight models downloaded, because nothing ever reconciled the two.
///
/// Exact match wins; otherwise the first installed tag whose base name (the
/// part before `:`) matches, preferring the shortest remaining suffix so
/// `qwen2.5:7b-instruct-q8_0` beats `qwen2.5:7b-instruct-q8_0-something`.
pub fn resolve_model(desired: &str) -> Option<String> {
    let installed = installed_tags();
    if installed.iter().any(|t| t == desired) {
        return Some(desired.to_string());
    }

    let want_base = desired.split(':').next().unwrap_or(desired);
    let want_size = desired.split(':').nth(1).unwrap_or("");

    let mut candidates: Vec<&String> = installed
        .iter()
        .filter(|t| t.split(':').next().unwrap_or("") == want_base)
        .collect();

    // Prefer tags that also carry the requested size ("7b", "9b"), so asking
    // for qwen2.5:7b never quietly resolves to a 32b that won't fit.
    if !want_size.is_empty() {
        let sized: Vec<&String> = candidates
            .iter()
            .filter(|t| t.split(':').nth(1).is_some_and(|v| v.starts_with(want_size)))
            .copied()
            .collect();
        if !sized.is_empty() {
            candidates = sized;
        }
    }

    candidates.sort_by_key(|t| t.len());
    candidates.first().map(|t| t.to_string())
}

/// Minimal blocking GET (the crate already depends on reqwest's blocking API).
fn ureq_get(url: &str) -> Result<String, String> {
    reqwest::blocking::Client::new()
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())
}

pub fn is_running() -> bool {
    OLLAMA_ADDR
        .parse::<SocketAddr>()
        .ok()
        .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok())
        .unwrap_or(false)
}

/// Ensure an Ollama server is up. If the port is dead, spawn `ollama serve`
/// detached and poll until it answers (up to ~15s). Idempotent and cheap when a
/// server is already running.
pub fn ensure_running() -> Result<(), String> {
    if is_running() {
        return Ok(());
    }
    let bin = ollama_bin().ok_or_else(|| {
        "Ollama is not installed. Install it from https://ollama.com/download".to_string()
    })?;
    Command::new(&bin)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start `ollama serve`: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if is_running() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err("Ollama did not become ready in time".into())
}
