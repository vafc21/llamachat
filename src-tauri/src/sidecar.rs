//! Bridge to the Python benchmark sidecar.
//!
//! Phase 1 invokes the sidecar as one-shot subcommands (`list-adapters`,
//! `list-models`, `benchmark`) and parses the single JSON object it prints on
//! stdout. This keeps the integration robust and matches the CLI contract in
//! `CONTRACT.md`. A long-lived `serve` (JSON-RPC) mode also exists in the
//! sidecar for future streaming/progress work.
//!
//! In dev we run the sidecar with the system Python from the repo's `sidecar/`
//! directory. Packaging it as a frozen single-file binary (PyInstaller) is a
//! Phase 2 concern; see the README.

use anyhow::{anyhow, Context, Result};
use fitllm_core::BenchmarkResult;
use std::path::PathBuf;
use std::process::Command;

/// Locate the `sidecar/` directory relative to the running binary or the repo.
fn sidecar_dir() -> Option<PathBuf> {
    // 1. Env override (set by packagers).
    if let Ok(p) = std::env::var("FITLLM_SIDECAR_DIR") {
        let pb = PathBuf::from(p);
        if pb.join("fitllm_sidecar").is_dir() {
            return Some(pb);
        }
    }
    // 2. Walk up from the current exe and cwd looking for `sidecar/fitllm_sidecar`.
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        candidates.extend(exe.ancestors().map(|a| a.join("sidecar")));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("sidecar"));
        candidates.push(cwd.join("../sidecar"));
    }
    candidates
        .into_iter()
        .find(|c| c.join("fitllm_sidecar").is_dir())
}

fn python() -> String {
    std::env::var("FITLLM_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// Run one sidecar subcommand and return its stdout.
fn run(args: &[&str]) -> Result<String> {
    let dir = sidecar_dir().ok_or_else(|| anyhow!("sidecar directory not found"))?;
    let out = Command::new(python())
        .arg("-m")
        .arg("fitllm_sidecar")
        .args(args)
        .current_dir(&dir)
        .output()
        .with_context(|| "failed to spawn python sidecar")?;
    if !out.status.success() {
        return Err(anyhow!(
            "sidecar exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Run a quick benchmark for one Ollama model tag. Returns a `BenchmarkResult`
/// even on failure (with `ok: false`) so the caller can surface the reason.
pub fn quick_benchmark(model: &str) -> Result<BenchmarkResult> {
    let stdout = run(&["benchmark", "--adapter", "ollama", "--model", model, "--tier", "quick"])?;
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| anyhow!("no JSON in sidecar output: {stdout}"))?;
    Ok(serde_json::from_str(line)?)
}

/// Ask the sidecar which locally-installed Ollama models are available.
pub fn list_models() -> Result<Vec<String>> {
    let stdout = run(&["list-models", "--adapter", "ollama"])?;
    let v: serde_json::Value = serde_json::from_str(stdout.trim())?;
    Ok(v.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

/// Is the Ollama adapter available (server reachable)?
pub fn ollama_available() -> bool {
    let Ok(stdout) = run(&["list-adapters"]) else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(stdout.trim())
        .ok()
        .and_then(|v| {
            v.get("adapters").and_then(|a| a.as_array()).map(|arr| {
                arr.iter().any(|ad| {
                    ad.get("name").and_then(|n| n.as_str()) == Some("ollama")
                        && ad.get("available").and_then(|b| b.as_bool()) == Some(true)
                })
            })
        })
        .unwrap_or(false)
}
