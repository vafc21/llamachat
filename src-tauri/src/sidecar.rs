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
use llamachat_core::BenchmarkResult;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Locate the `sidecar/` directory relative to the running binary or the repo.
fn sidecar_dir() -> Option<PathBuf> {
    // 1. Env override (set by packagers).
    if let Ok(p) = std::env::var("FITLLM_SIDECAR_DIR") {
        let pb = PathBuf::from(p);
        if pb.join("llamachat_sidecar").is_dir() {
            return Some(pb);
        }
    }
    // 2. Walk up from the current exe and cwd looking for `sidecar/llamachat_sidecar`.
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
        .find(|c| c.join("llamachat_sidecar").is_dir())
}

fn python() -> String {
    std::env::var("FITLLM_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// Locate the bundled, frozen `llamachat-sidecar` binary that ships next to the app
/// executable in a packaged build (`LlamaChat.app/Contents/MacOS/llamachat-sidecar`).
/// Packagers can point at it explicitly with `FITLLM_SIDECAR_BIN`.
fn bundled_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FITLLM_SIDECAR_BIN") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let name = if cfg!(windows) { "llamachat-sidecar.exe" } else { "llamachat-sidecar" };
    let cand = exe.parent()?.join(name);
    cand.is_file().then_some(cand)
}

/// Build a `Command` that runs the sidecar, preferring the bundled frozen binary
/// and falling back to `python -m llamachat_sidecar` from the repo `sidecar/` dir in
/// dev. The frozen binary takes args straight through; the Python fallback needs
/// the module invocation and its working directory.
fn sidecar_command() -> Result<Command> {
    if let Some(bin) = bundled_bin() {
        return Ok(Command::new(bin));
    }
    let dir = sidecar_dir()
        .ok_or_else(|| anyhow!("sidecar not found: no bundled binary and no sidecar/ dir"))?;
    let mut c = Command::new(python());
    c.arg("-m").arg("llamachat_sidecar").current_dir(&dir);
    Ok(c)
}

/// Run one sidecar subcommand and return its stdout.
fn run(args: &[&str]) -> Result<String> {
    let out = sidecar_command()?
        .args(args)
        .output()
        .with_context(|| "failed to spawn sidecar")?;
    if !out.status.success() {
        return Err(anyhow!(
            "sidecar exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Benchmark one Ollama model tag at the given tier (`quick` | `balanced` |
/// `full`). The tier is passed straight through to the Python sidecar. Returns
/// a `BenchmarkResult` even on failure (with `ok: false`) so the caller can
/// surface the reason.
pub fn benchmark(model: &str, tier: &str) -> Result<BenchmarkResult> {
    let stdout = run(&["benchmark", "--adapter", "ollama", "--model", model, "--tier", tier])?;
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| anyhow!("no JSON in sidecar output: {stdout}"))?;
    Ok(serde_json::from_str(line)?)
}

/// Run a quick benchmark for one Ollama model tag. Thin wrapper over
/// [`benchmark`] kept as a stable helper for existing callers.
#[allow(dead_code)]
pub fn quick_benchmark(model: &str) -> Result<BenchmarkResult> {
    benchmark(model, "quick")
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

/// Chat with a model via the sidecar's serve mode. Streams tokens through a callback.
/// Returns the full response text on success.
pub fn chat(
    model: &str,
    messages: &[serde_json::Value],
    system: &str,
    on_token: impl Fn(&str),
) -> Result<String> {
    let mut child = sidecar_command()?
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // keep stderr separate
        .spawn()
        .context("failed to spawn sidecar serve process")?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    // Send the chat request
    let request = serde_json::json!({
        "id": 1,
        "method": "chat",
        "params": {
            "adapter": "ollama",
            "model": model,
            "messages": messages,
            "system": system,
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&request)?)?;
    drop(stdin); // close stdin to signal end of requests

    // Read streaming responses
    let mut full = String::new();
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Value = serde_json::from_str(&line)?;

        // Handle streaming token events
        if obj.get("event").and_then(|e| e.as_str()) == Some("token") {
            if let Some(token) = obj.get("token").and_then(|t| t.as_str()) {
                full.push_str(token);
                on_token(token);
            }
            continue;
        }

        // Handle final result
        if obj.get("id").is_some() && obj.get("result").is_some() {
            break;
        }
    }

    child.wait().ok();
    Ok(full)
}
