//! Ollama process + path management.
//!
//! A GUI-launched macOS app inherits a minimal PATH (typically `/usr/bin:/bin`)
//! that does NOT include Homebrew's `/opt/homebrew/bin`, so `Command::new("ollama")`
//! fails when the app is launched from `/Applications`. We resolve the binary's
//! absolute path here, and make sure a server is actually listening before we
//! pull a model or chat — spawning `ollama serve` ourselves if it isn't.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const OLLAMA_ADDR: &str = "127.0.0.1:11434";

/// Absolute path to the `ollama` binary. Searches an env override, the common
/// Homebrew / `/usr/local` install locations, and finally the inherited PATH so
/// terminal/dev launches keep working too.
pub fn ollama_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FITLLM_OLLAMA_BIN") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    for c in [
        "/opt/homebrew/bin/ollama",
        "/usr/local/bin/ollama",
        "/usr/bin/ollama",
        "/opt/homebrew/opt/ollama/bin/ollama",
    ] {
        let pb = PathBuf::from(c);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join("ollama");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// True when an Ollama server is accepting connections on the default port.
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
