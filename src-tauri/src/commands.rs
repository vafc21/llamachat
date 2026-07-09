//! Tauri IPC commands — the bridge between the React UI and the core engine.
//! Command names and payload shapes match `CONTRACT.md`.

use crate::sidecar;
use crate::state::{data_dir, AppState};
use fitllm_core::{hardware, recommend, HardwareProfile, ModelCatalog, Recommendation};
use fitllm_core::tools::ToolRequest;
use tauri::{Emitter, Manager, State};


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
    let path = data_dir().join("consent");
    if granted {
        std::fs::write(path, chrono::Utc::now().to_rfc3339()).ok();
    } else {
        std::fs::remove_file(path).ok();
    }
    Ok(())
}

/// Kick off the non-blocking quick benchmark. Emits `benchmark_progress`
/// events as it goes and a final `recommendations_updated` with fresh,
/// measured recommendations. Returns immediately.
#[tauri::command]
pub fn start_quick_benchmark(app: tauri::AppHandle) {
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

        // Pick candidate models: catalog models whose default tag is installed
        // locally, capped to a few so the quick pass stays light.
        let installed = sidecar::list_models().unwrap_or_default();
        let state = app.state::<AppState>();
        let candidates: Vec<String> = {
            let inner = state.0.lock().unwrap();
            inner
                .catalog
                .models
                .iter()
                .map(|m| m.ollama_pull.clone())
                .filter(|tag| installed.iter().any(|i| i == tag || i.starts_with(tag)))
                .take(4)
                .collect()
        };

        if candidates.is_empty() {
            emit("no-models", 100, "");
            return;
        }

        let total = candidates.len();
        for (i, model) in candidates.iter().enumerate() {
            let pct = 10 + (80 * i as u32 / total as u32);
            emit("benchmarking", pct, model);
            if let Ok(result) = sidecar::quick_benchmark(model) {
                let mut inner = state.0.lock().unwrap();
                inner.store.save_benchmark(&result).ok();
                inner.benchmarks.push(result);
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
    std::fs::remove_file(dir.join("fitllm.db")).ok();
    std::fs::remove_file(dir.join("consent")).ok();
    inner.consent_granted = false;
    // Reopen a fresh store so the app keeps working after a wipe.
    if let Ok(store) = fitllm_core::store::Store::open(&dir.join("fitllm.db")) {
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
    message: String,
) -> Result<(), String> {
    let (model_tag, messages, system_prompt) = {
        let inner = state.0.lock().map_err(|e| e.to_string())?;
        let profile = inner.profile.clone();
        let catalog = inner.catalog.clone();
        let benchmarks = inner.benchmarks.clone();
        let sys = inner.tools.system_prompt();

        // Pick best model from recommendations, or fall back
        let model = if let Some(p) = profile {
            let recs = recommend::rate_all(&p, &catalog, &benchmarks);
            recs.first()
                .map(|r| r.ollama_pull.clone())
                .unwrap_or_else(|| "llama3.2:1b".into())
        } else {
            "llama3.2:1b".into()
        };

        let msgs = vec![serde_json::json!({"role": "user", "content": message})];
        (model, msgs, sys)
    };

    // Run in background thread to not block the main thread
    std::thread::spawn(move || {
        let _ = sidecar::chat(&model_tag, &messages, &system_prompt, |token| {
            let _ = app.emit("chat_token", token.to_string());
        });
        let _ = app.emit("chat_done", true);
    });

    Ok(())
}
