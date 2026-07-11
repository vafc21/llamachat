//! App-level settings and user-defined ("custom") models.
//!
//! These are FitLLM shell concerns (not shared core types), so they live in the
//! Tauri crate. Both are persisted as plain JSON files in the app `data_dir()`
//! so they survive restarts and never phone home.

use fitllm_core::{CatalogModel, ModelCatalog, Quant};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Persisted user preferences. JSON shape matches `CONTRACT.md` / the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Benchmark thoroughness: "quick" | "balanced" | "full".
    pub benchmark_intensity: String,
    /// If set, forces this model tag instead of the top recommendation.
    pub model_override: Option<String>,
    /// Override directory where models are stored.
    pub models_dir: Option<String>,
    /// Override directory where chats + memory.md are stored (markdown files).
    /// `None` = the app data dir.
    #[serde(default)]
    pub memory_dir: Option<String>,
    /// How Agent mode perceives the screen: "accessibility" (read UI as text —
    /// works with text models) or "vision" (screenshot → vision model describes).
    #[serde(default = "default_perception")]
    pub perception: String,
    /// Vision model to describe screenshots when perception = "vision".
    #[serde(default)]
    pub vision_model: Option<String>,
    /// When true (the default), no telemetry is ever collected.
    pub telemetry_off: bool,
}

fn default_perception() -> String {
    "accessibility".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            benchmark_intensity: "balanced".to_string(),
            model_override: None,
            models_dir: None,
            memory_dir: None,
            perception: default_perception(),
            vision_model: None,
            telemetry_off: true,
        }
    }
}

/// The file backing [`AppSettings`], under `data_dir()`.
fn settings_path(dir: &Path) -> std::path::PathBuf {
    dir.join("settings.json")
}

/// Load persisted settings, falling back to defaults on any error.
pub fn load_settings(dir: &Path) -> AppSettings {
    std::fs::read_to_string(settings_path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist settings as pretty JSON.
pub fn save_settings(dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(dir), json).map_err(|e| e.to_string())
}

/// User-supplied fields for a custom model (from the UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModelInput {
    pub display_name: String,
    pub ollama_pull: String,
    pub params_b: f64,
    pub quality_score: Option<f64>,
    pub context_default: Option<u32>,
}

/// The file backing the custom-model list, under `data_dir()`.
fn custom_models_path(dir: &Path) -> std::path::PathBuf {
    dir.join("custom_models.json")
}

/// Load the persisted custom models (already normalized to `CatalogModel`).
pub fn load_custom_models(dir: &Path) -> Vec<CatalogModel> {
    std::fs::read_to_string(custom_models_path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist the custom-model list as pretty JSON.
pub fn save_custom_models(dir: &Path, models: &[CatalogModel]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(models).map_err(|e| e.to_string())?;
    std::fs::write(custom_models_path(dir), json).map_err(|e| e.to_string())
}

/// Turn a human display name into a stable, filesystem/id-safe slug.
/// Prefixed with `custom-` so user models never collide with catalog ids.
pub fn slug(display_name: &str) -> String {
    let mut out = String::from("custom-");
    let mut last_dash = true; // avoids a leading dash right after the prefix
    for ch in display_name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    if out == "custom" {
        out.push_str("-model");
    }
    out
}

/// Convert user input into a full [`CatalogModel`] so the recommender can rate
/// it exactly like a built-in. Sizes a single Q4_K_M quant from `params_b`.
pub fn to_catalog_model(input: &CustomModelInput) -> CatalogModel {
    // ~0.6 GB per billion params at Q4_K_M (weights only), in MB.
    let size_mb = ((input.params_b * 0.6 * 1024.0).round() as u64).max(1);
    let context = input.context_default.unwrap_or(4096);
    CatalogModel {
        id: slug(&input.display_name),
        family: "Custom".to_string(),
        display_name: input.display_name.clone(),
        params_b: input.params_b,
        license: "custom".to_string(),
        quality_score: input.quality_score.unwrap_or(50.0),
        quality_source: "user-provided".to_string(),
        context_default: context,
        context_max: context,
        quants: vec![Quant {
            name: "Q4_K_M".to_string(),
            bits: 4.5,
            size_mb,
            ollama_tag: Some(input.ollama_pull.clone()),
        }],
        ollama_pull: input.ollama_pull.clone(),
        tags: vec!["custom".to_string()],
    }
}

/// Build the effective catalog = bundled base + user custom models.
pub fn merged_catalog(base: &ModelCatalog, custom: &[CatalogModel]) -> ModelCatalog {
    let mut cat = base.clone();
    cat.models.extend(custom.iter().cloned());
    cat
}
