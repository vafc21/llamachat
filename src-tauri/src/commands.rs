//! Tauri IPC commands — the bridge between the React UI and the core engine.
//! Command names and payload shapes match `CONTRACT.md`.

use crate::memory::{self, ConvDto};
use crate::ollama;
use crate::settings::{self, AppSettings, CustomModelInput};
use crate::sidecar;
use crate::state::{data_dir, AppState};
use llamachat_core::{hardware, recommend, HardwareProfile, LevelPlan, ModelCatalog, Recommendation};
use llamachat_core::tools::ToolRequest;
use std::io::Read;
use std::process::{Command, Stdio};
use tauri::{Emitter, Manager, State};

/// Plain conversational system prompt used for chat when the caller doesn't
/// supply one (e.g. from a skill or `/system`).
const DEFAULT_CHAT_PROMPT: &str = "You are LlamaChat, a helpful AI assistant running locally on the \
    user's machine. Reply directly and conversationally in plain language. Be concise and accurate. \
    Do not output JSON, function calls, or tool syntax unless the user explicitly asks for code.";


/// Detect (or return the cached) hardware profile. Read-only.
#[tauri::command]
pub fn get_hardware_profile(state: State<AppState>) -> Result<HardwareProfile, String> {
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(p) = &inner.profile {
        return Ok(p.clone());
    }
    let profile = hardware::profile().map_err(|e| e.to_string())?;
    inner.store.save_profile(&profile).ok();
    inner.profile = Some(profile.clone());
    Ok(profile)
}

/// Current recommendations, blending any measured benchmarks with heuristics.
#[tauri::command]
pub fn get_recommendations(state: State<AppState>) -> Result<Vec<Recommendation>, String> {
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    if inner.profile.is_none() {
        let p = hardware::profile().map_err(|e| e.to_string())?;
        inner.store.save_profile(&p).ok();
        inner.profile = Some(p);
    }
    let profile = inner.profile.clone().unwrap();
    Ok(recommend::rate_all(&profile, &inner.catalog, &inner.benchmarks))
}

/// Per-level model plan: which model each tier (Quick/Standard/Max) will run on
/// THIS machine, sized to the hardware, so the UI can show the model + its
/// intelligence/speed scores at each tier *before* the user commits — instead of
/// one consolidated picker. See `docs/design/benchmark-levels.md`.
#[tauri::command]
pub fn get_benchmark_plan(state: State<AppState>) -> Result<LevelPlan, String> {
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    if inner.profile.is_none() {
        let p = hardware::profile().map_err(|e| e.to_string())?;
        inner.store.save_profile(&p).ok();
        inner.profile = Some(p);
    }
    let profile = inner.profile.clone().unwrap();
    Ok(recommend::plan_levels(&profile, &inner.catalog, &inner.benchmarks))
}

#[tauri::command]
pub fn get_catalog(state: State<AppState>) -> Result<ModelCatalog, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    Ok(inner.catalog.clone())
}

#[tauri::command]
pub fn get_consent(state: State<AppState>) -> Result<bool, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    Ok(inner.consent_granted)
}

#[tauri::command]
pub fn set_consent(state: State<AppState>, granted: bool) -> Result<(), String> {
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    inner.consent_granted = granted;
    // Keep the tool registry in sync so destructive tools (shell) unlock immediately.
    inner.tools.set_destructive_allowed(granted);
    let path = data_dir().join("consent");
    if granted {
        std::fs::write(path, chrono::Utc::now().to_rfc3339()).ok();
    } else {
        std::fs::remove_file(path).ok();
    }
    Ok(())
}

/// Kick off the non-blocking quick benchmark. Kept for the tray and existing
/// callers; delegates to [`run_benchmark`] at the "quick" tier.
#[tauri::command]
pub fn start_quick_benchmark(app: tauri::AppHandle) {
    run_benchmark(app, "quick".to_string());
}

/// Kick off a non-blocking benchmark at the requested intensity
/// (`quick` | `balanced` | `full`). The tier is passed through to the sidecar.
#[tauri::command]
pub fn start_benchmark(app: tauri::AppHandle, intensity: String) {
    run_benchmark(app, intensity);
}

/// Shared benchmark driver. Emits `benchmark_progress` events as it goes and a
/// final `recommendations_updated` with fresh, measured recommendations.
/// Returns immediately.
fn run_benchmark(app: tauri::AppHandle, tier: String) {
    std::thread::spawn(move || {
        let emit = |stage: &str, pct: u32, model: &str| {
            app.emit(
                "benchmark_progress",
                serde_json::json!({ "stage": stage, "pct": pct, "model": model }),
            )
            .ok();
        };

        emit("checking-runtime", 5, "");
        if !sidecar::ollama_available() {
            emit("no-runtime", 100, "");
            app.emit(
                "benchmark_progress",
                serde_json::json!({
                    "stage": "no-runtime", "pct": 100, "model": "",
                    "detail": "Ollama not reachable — install it and run `ollama serve` for measured ratings."
                }),
            )
            .ok();
            return;
        }

        // Choose WHICH models to benchmark from the hardware-sized level plan —
        // not "whatever is installed". This is what stops Full/Max from
        // underestimating a strong machine (e.g. an M4 getting a 3B). The level
        // names the model; measurement depth is a separate knob derived below.
        let installed = sidecar::list_models().unwrap_or_default();
        let state = app.state::<AppState>();
        let (targets, depths): (Vec<Recommendation>, Vec<String>) = {
            let mut inner = state.0.lock().unwrap();
            if inner.profile.is_none() {
                if let Ok(p) = hardware::profile() {
                    inner.store.save_profile(&p).ok();
                    inner.profile = Some(p);
                }
            }
            let plan = inner
                .profile
                .clone()
                .map(|p| recommend::plan_levels(&p, &inner.catalog, &inner.benchmarks));
            // Level = which model(s). Depth = how thorough the measurement is.
            // "all" is the all-against-all matrix: every fitting model measured at
            // EVERY depth, one result per (model, depth) cell.
            let depths: Vec<String> = match tier.as_str() {
                "quick" => vec!["quick".into()],
                "balanced" | "standard" => vec!["balanced".into()],
                "all" => vec!["quick".into(), "balanced".into(), "full".into()],
                _ => vec!["full".into()], // full / max
            };
            // Each setting runs and reports a COHORT of models — not a single
            // pick. Quick = the fast set, Standard = the Great+ set, Full/Max/All
            // = every model that fits (so Full exercises the big models too).
            let targets = match (plan, tier.as_str()) {
                (Some(p), "quick") => p.quick_set,
                (Some(p), "balanced") | (Some(p), "standard") => p.standard_set,
                (Some(p), _) => p.all, // full / max / all
                (None, _) => Vec::new(),
            };
            (targets, depths)
        };

        if targets.is_empty() {
            emit("no-models", 100, "");
            return;
        }

        // Benchmark each model at each requested depth. For "all" that's the full
        // model×intensity grid. If a model isn't installed, tell the UI to offer a
        // download — never silently fall back to a smaller installed model (that
        // silent downgrade was the original bug).
        let total = (targets.len() * depths.len()).max(1);
        let mut done = 0usize;
        for target in targets.iter() {
            let tag = &target.ollama_pull;
            let is_installed = installed
                .iter()
                .any(|iname| iname == tag || iname.starts_with(tag.as_str()));
            if !is_installed {
                let pct = 10 + (80 * done as u32 / total as u32);
                app.emit(
                    "benchmark_progress",
                    serde_json::json!({
                        "stage": "needs-download", "pct": pct, "model": tag,
                        "detail": format!(
                            "{} isn't installed yet — download it to benchmark this tier.",
                            target.display_name
                        )
                    }),
                )
                .ok();
                done += depths.len();
                continue;
            }
            for depth in depths.iter() {
                let pct = 10 + (80 * done as u32 / total as u32);
                emit("benchmarking", pct, tag);
                if let Ok(result) = sidecar::benchmark(tag, depth) {
                    let mut inner = state.0.lock().unwrap();
                    inner.store.save_benchmark(&result).ok();
                    inner.benchmarks.push(result);
                }
                done += 1;
            }
        }

        // Recompute measured recommendations and push them to the UI.
        emit("finalizing", 95, "");
        let recs = {
            let inner = state.0.lock().unwrap();
            let profile = inner.profile.clone();
            profile.map(|p| recommend::rate_all(&p, &inner.catalog, &inner.benchmarks))
        };
        if let Some(recs) = recs {
            app.emit("recommendations_updated", recs).ok();
        }
        emit("done", 100, "");
    });
}

/// Export all locally-stored data (profile + benchmark history) as JSON.
#[tauri::command]
pub fn export_data(state: State<AppState>) -> Result<serde_json::Value, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "profile": inner.profile,
        "benchmarks": inner.benchmarks,
        "exported_at": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Wipe everything: delete the local database and reset in-memory state.
#[tauri::command]
pub fn wipe_data(state: State<AppState>) -> Result<(), String> {
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    inner.benchmarks.clear();
    inner.profile = None;
    let dir = data_dir();
    std::fs::remove_file(dir.join("llamachat.db")).ok();
    std::fs::remove_file(dir.join("consent")).ok();
    inner.consent_granted = false;
    // Reopen a fresh store so the app keeps working after a wipe.
    if let Ok(store) = llamachat_core::store::Store::open(&dir.join("llamachat.db")) {
        inner.store = store;
    }
    Ok(())
}

// ── Tool system commands ──────────────────────────────────────

/// List all available tools with their parameter schemas.
#[tauri::command]
pub fn list_tools(state: State<AppState>) -> Result<serde_json::Value, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!(inner.tools.list_tools()))
}

/// Execute a tool and return the result. Destructive tools may require approval.
#[tauri::command]
pub fn execute_tool(state: State<AppState>, request: ToolRequest) -> Result<serde_json::Value, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    let result = inner.tools.execute(&request);
    Ok(serde_json::json!(result))
}

/// Check if a tool needs user approval before execution.
#[tauri::command]
pub fn tool_needs_approval(state: State<AppState>, tool_name: String) -> Result<bool, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    Ok(inner.tools.needs_approval(&tool_name))
}

/// Generate a system prompt describing available tools for the model.
#[tauri::command]
pub fn get_tool_system_prompt(state: State<AppState>) -> Result<String, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    Ok(inner.tools.system_prompt())
}

/// Send a message to the model and stream the response back via events.
/// Emits `chat_token` events for each token and a final `chat_done` event.
#[tauri::command]
pub fn send_message(
    app: tauri::AppHandle,
    state: State<AppState>,
    messages: Vec<serde_json::Value>,
    model: Option<String>,
    system: Option<String>,
) -> Result<(), String> {
    let (model_tag, system_prompt) = {
        let inner = state.0.lock().map_err(|e| e.to_string())?;
        let profile = inner.profile.clone();
        let catalog = inner.catalog.clone();
        let benchmarks = inner.benchmarks.clone();

        // System prompt: caller override (a skill or /system), else a plain
        // conversational default. The tool-calling prompt is NOT used here — it
        // makes models emit raw tool JSON for every message. Long-term memory
        // (memory.md) is auto-injected so the model always "knows" saved facts.
        let base_sys = match system {
            Some(s) if !s.trim().is_empty() => s,
            _ => DEFAULT_CHAT_PROMPT.to_string(),
        };
        let mem = memory::read_memory(&inner.settings.memory_dir);
        let sys = if mem.trim().is_empty() {
            base_sys
        } else {
            format!(
                "{base_sys}\n\nWhat you know about the user (their saved memory — use it when relevant, don't recite it verbatim):\n{}",
                mem.trim()
            )
        };

        // Honour the model the UI selected (the picker); otherwise pick the best
        // model from recommendations, or fall back to a tiny default.
        let model = match model {
            Some(m) if !m.trim().is_empty() => m,
            _ => {
                if let Some(p) = profile {
                    let recs = recommend::rate_all(&p, &catalog, &benchmarks);
                    recs.first()
                        .map(|r| r.ollama_pull.clone())
                        .unwrap_or_else(|| "llama3.2:1b".into())
                } else {
                    "llama3.2:1b".into()
                }
            }
        };

        (model, sys)
    };

    // Run in background thread to not block the main thread.
    std::thread::spawn(move || {
        // Make sure Ollama is reachable before we try to chat.
        if let Err(e) = ollama::ensure_running() {
            let _ = app.emit("chat_token", format!("⚠️ {e}"));
            let _ = app.emit("chat_done", true);
            return;
        }
        if let Err(e) = sidecar::chat(&model_tag, &messages, &system_prompt, |token| {
            let _ = app.emit("chat_token", token.to_string());
        }) {
            let _ = app.emit(
                "chat_token",
                format!(
                    "⚠️ Couldn't reach the model \"{model_tag}\": {e}\n\nIt may still be \
                     downloading, or Ollama isn't running yet — try again in a moment."
                ),
            );
        }
        let _ = app.emit("chat_done", true);
    });

    Ok(())
}

/// Which Ollama models are already downloaded locally (by tag), so the UI can
/// show tier readiness without re-pulling. Ensures the server is up first.
#[tauri::command]
pub fn list_installed_models() -> Result<Vec<String>, String> {
    ollama::ensure_running().ok();
    sidecar::list_models().map_err(|e| e.to_string())
}

// ── Markdown memory: chats + long-term memory.md ──────────────

/// Persist one chat as a markdown transcript under the memory dir.
#[tauri::command]
pub fn save_conversation(state: State<AppState>, conversation: ConvDto) -> Result<(), String> {
    let dir = state.0.lock().map_err(|e| e.to_string())?.settings.memory_dir.clone();
    memory::save_conversation(&dir, &conversation)
}

/// Load every saved chat (newest first).
#[tauri::command]
pub fn list_conversations(state: State<AppState>) -> Result<Vec<ConvDto>, String> {
    let dir = state.0.lock().map_err(|e| e.to_string())?.settings.memory_dir.clone();
    Ok(memory::list_conversations(&dir))
}

/// Delete a saved chat's markdown file.
#[tauri::command]
pub fn delete_conversation(state: State<AppState>, id: String) -> Result<(), String> {
    let dir = state.0.lock().map_err(|e| e.to_string())?.settings.memory_dir.clone();
    memory::delete_conversation(&dir, &id)
}

/// Read the long-term memory.md (facts injected into every chat).
#[tauri::command]
pub fn get_memory(state: State<AppState>) -> Result<String, String> {
    let dir = state.0.lock().map_err(|e| e.to_string())?.settings.memory_dir.clone();
    Ok(memory::read_memory(&dir))
}

/// Replace the long-term memory.md.
#[tauri::command]
pub fn set_memory(state: State<AppState>, content: String) -> Result<(), String> {
    let dir = state.0.lock().map_err(|e| e.to_string())?.settings.memory_dir.clone();
    memory::write_memory(&dir, &content)
}

/// The resolved memory root directory (for display in Settings).
#[tauri::command]
pub fn get_memory_dir(state: State<AppState>) -> Result<String, String> {
    let dir = state.0.lock().map_err(|e| e.to_string())?.settings.memory_dir.clone();
    Ok(memory::root(&dir).to_string_lossy().to_string())
}

// ── Agent mode (tool-use loop) ────────────────────────────────

/// Start an agent run in the background. `mode` is plan | ask | auto | bypass.
/// Emits agent_* events; returns immediately.
#[tauri::command]
pub fn run_agent(
    app: tauri::AppHandle,
    state: State<AppState>,
    messages: Vec<serde_json::Value>,
    model: Option<String>,
    mode: String,
) -> Result<(), String> {
    let model_tag = {
        let inner = state.0.lock().map_err(|e| e.to_string())?;
        match model {
            Some(m) if !m.trim().is_empty() => m,
            _ => inner
                .profile
                .clone()
                .map(|p| {
                    recommend::rate_all(&p, &inner.catalog, &inner.benchmarks)
                        .first()
                        .map(|r| r.ollama_pull.clone())
                        .unwrap_or_else(|| "llama3.2:1b".into())
                })
                .unwrap_or_else(|| "llama3.2:1b".into()),
        }
    };
    std::thread::spawn(move || crate::agent::run(app, messages, model_tag, mode));
    Ok(())
}

/// Answer an Ask-mode approval prompt.
#[tauri::command]
pub fn approve_agent(state: State<AppState>, approved: bool) -> Result<(), String> {
    state.0.lock().map_err(|e| e.to_string())?.agent_decision = Some(approved);
    Ok(())
}

/// Stop the running agent after the current step.
#[tauri::command]
pub fn stop_agent(state: State<AppState>) -> Result<(), String> {
    state.0.lock().map_err(|e| e.to_string())?.agent_stop = true;
    Ok(())
}

// ── Permissions (agent setup page) ────────────────────────────

#[cfg(target_os = "macos")]
fn accessibility_trusted() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}
#[cfg(not(target_os = "macos"))]
fn accessibility_trusted() -> bool {
    // Windows/Linux don't gate agent mouse/keyboard input behind a macOS-style
    // Accessibility permission — treat as available.
    true
}

// Screen Recording (TCC) — CoreGraphics exposes the same calls the OS uses to
// gate `screencapture`. Preflight reads current status without prompting;
// Request pops the system prompt once AND registers the app in the
// Privacy ▸ Screen Recording list (which is why the app wasn't appearing there:
// nothing had ever called this).
#[cfg(target_os = "macos")]
mod screen_capture {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    pub fn preflight() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }
    pub fn request() -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
    }
}

#[cfg(target_os = "macos")]
fn screen_recording_granted() -> bool {
    screen_capture::preflight()
}
#[cfg(not(target_os = "macos"))]
fn screen_recording_granted() -> bool {
    // Windows/Linux screen capture isn't permission-gated the way macOS is.
    true
}

/// Status of the things Agent mode needs, for the setup checklist.
#[tauri::command]
pub fn check_permissions() -> serde_json::Value {
    serde_json::json!({
        "accessibility": accessibility_trusted(),
        "screen_recording": screen_recording_granted(),
        "ollama": ollama::is_running(),
    })
}

/// Prompt for Accessibility permission; returns whether it's now granted.
#[tauri::command]
pub fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
    }
    #[cfg(not(target_os = "macos"))]
    {
        // No prompt needed on Windows/Linux.
        true
    }
}

/// Prompt for Screen Recording permission (screenshot-vision mode). The first
/// call pops the macOS prompt and registers the app under Privacy ▸ Screen
/// Recording; later calls just return the current status. Returns whether granted.
#[tauri::command]
pub fn request_screen_recording() -> bool {
    #[cfg(target_os = "macos")]
    {
        screen_capture::request()
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Not permission-gated on Windows/Linux.
        true
    }
}

/// Open a macOS Privacy settings pane ("accessibility" | "screen_recording" | "automation").
#[tauri::command]
pub fn open_settings_pane(pane: String) -> Result<(), String> {
    let url = match pane.as_str() {
        "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "screen_recording" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        "automation" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation",
        _ => return Err(format!("unknown pane: {pane}")),
    };
    Command::new("open").arg(url).status().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Settings ──────────────────────────────────────────────────

/// Return the current persisted app settings.
#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    let inner = state.0.lock().map_err(|e| e.to_string())?;
    Ok(inner.settings.clone())
}

/// Replace and persist the app settings.
#[tauri::command]
pub fn set_settings(state: State<AppState>, settings: AppSettings) -> Result<(), String> {
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    settings::save_settings(&data_dir(), &settings)?;
    inner.settings = settings;
    Ok(())
}

// ── Custom models ─────────────────────────────────────────────

/// Add a user-defined model. It is normalized into a full catalog entry (so the
/// recommender rates it like a built-in), persisted, and merged into the
/// effective catalog. Returns the stable id assigned to it.
#[tauri::command]
pub fn add_custom_model(state: State<AppState>, model: CustomModelInput) -> Result<String, String> {
    let entry = settings::to_catalog_model(&model);
    let id = entry.id.clone();
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    // Replace any existing custom model with the same id (idempotent add).
    inner.custom_models.retain(|m| m.id != id);
    inner.custom_models.push(entry);
    settings::save_custom_models(&data_dir(), &inner.custom_models)?;
    inner.rebuild_catalog();
    Ok(id)
}

/// Remove a previously-added custom model by id.
#[tauri::command]
pub fn remove_custom_model(state: State<AppState>, id: String) -> Result<(), String> {
    let mut inner = state.0.lock().map_err(|e| e.to_string())?;
    inner.custom_models.retain(|m| m.id != id);
    settings::save_custom_models(&data_dir(), &inner.custom_models)?;
    inner.rebuild_catalog();
    Ok(())
}

// ── Model download ────────────────────────────────────────────

/// Pull an Ollama model in the background via `ollama pull <tag>`, streaming
/// progress to the UI as `download_progress` events with payload
/// `{ tag, pct, status, detail }` where `status` is
/// `"pulling"` | `"done"` | `"error"`. Returns immediately.
#[tauri::command]
pub fn download_model(app: tauri::AppHandle, tag: String) {
    std::thread::spawn(move || {
        let emit = |pct: Option<f64>, status: &str, detail: &str| {
            app.emit(
                "download_progress",
                serde_json::json!({
                    "tag": tag, "pct": pct, "status": status, "detail": detail,
                }),
            )
            .ok();
        };

        emit(None, "pulling", "Starting download…");

        // A GUI-launched app has a minimal PATH and the Ollama server may be
        // down; make sure it's running and resolve the binary's real path.
        if let Err(e) = ollama::ensure_running() {
            emit(None, "error", &e);
            return;
        }
        let Some(ollama_bin) = ollama::ollama_bin() else {
            emit(
                None,
                "error",
                "Ollama not found. Install it from https://ollama.com/download",
            );
            return;
        };

        let mut child = match Command::new(&ollama_bin)
            .arg("pull")
            .arg(&tag)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                emit(
                    None,
                    "error",
                    &format!("Failed to run `ollama pull`: {e}. Is Ollama installed?"),
                );
                return;
            }
        };

        // ollama writes its progress bar to stderr, updating the same line with
        // carriage returns; drain stdout on a side thread so it can't block.
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                let mut sink = Vec::new();
                let mut r = out;
                let _ = r.read_to_end(&mut sink);
            });
        }

        if let Some(err) = child.stderr.take() {
            for chunk in ProgressChunks::new(err) {
                let text = chunk.trim();
                if text.is_empty() {
                    continue;
                }
                emit(parse_percent(text), "pulling", text);
            }
        }

        match child.wait() {
            Ok(status) if status.success() => emit(Some(100.0), "done", "Download complete"),
            Ok(status) => emit(
                None,
                "error",
                &format!("`ollama pull` exited with {status}"),
            ),
            Err(e) => emit(None, "error", &format!("Failed waiting on ollama: {e}")),
        }
    });
}

/// Extract a percentage (0-100) from an ollama progress line like
/// `pulling abcd123... 47%`, if present.
fn parse_percent(line: &str) -> Option<f64> {
    for tok in line.split_whitespace() {
        if let Some(num) = tok.strip_suffix('%') {
            if let Ok(v) = num.parse::<f64>() {
                return Some(v.clamp(0.0, 100.0));
            }
        }
    }
    None
}

/// Reader adaptor that yields "lines" split on either `\n` or `\r`, so a
/// carriage-return-updated progress bar surfaces each update instead of
/// blocking until a newline.
struct ProgressChunks<R: Read> {
    inner: R,
    buf: Vec<u8>,
    byte: [u8; 1],
}

impl<R: Read> ProgressChunks<R> {
    fn new(inner: R) -> Self {
        ProgressChunks { inner, buf: Vec::new(), byte: [0u8; 1] }
    }
}

impl<R: Read> Iterator for ProgressChunks<R> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        loop {
            match self.inner.read(&mut self.byte) {
                Ok(0) => {
                    if self.buf.is_empty() {
                        return None;
                    }
                    let s = String::from_utf8_lossy(&self.buf).to_string();
                    self.buf.clear();
                    return Some(s);
                }
                Ok(_) => {
                    let b = self.byte[0];
                    if b == b'\n' || b == b'\r' {
                        if self.buf.is_empty() {
                            continue;
                        }
                        let s = String::from_utf8_lossy(&self.buf).to_string();
                        self.buf.clear();
                        return Some(s);
                    }
                    self.buf.push(b);
                }
                Err(_) => return None,
            }
        }
    }
}
