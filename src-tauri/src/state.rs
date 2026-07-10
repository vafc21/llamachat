//! Shared application state held behind a mutex and injected into every command.

use crate::settings::{self, AppSettings};
use fitllm_core::{
    catalog, store::Store,
    tools::{ToolLimits, ToolRegistry},
    BenchmarkResult, CatalogModel, HardwareProfile, ModelCatalog,
};
use fitllm_core::tools::{ShellTool, FilesystemTool, ProcessTool, DesktopTool};
use std::path::PathBuf;
use std::sync::Mutex;

/// Where FitLLM keeps its local SQLite store and settings.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("fitllm")
}

pub struct Inner {
    pub store: Store,
    /// Effective catalog = bundled base + user custom models. Everything
    /// downstream (recommendations, benchmark candidates) reads this.
    pub catalog: ModelCatalog,
    /// The bundled catalog exactly as shipped, kept so `catalog` can be rebuilt
    /// when the custom-model list changes.
    pub base_catalog: ModelCatalog,
    /// User-defined models, persisted to `custom_models.json`.
    pub custom_models: Vec<CatalogModel>,
    pub profile: Option<HardwareProfile>,
    /// All benchmark results loaded/collected this session, newest last.
    pub benchmarks: Vec<BenchmarkResult>,
    pub consent_granted: bool,
    /// Persisted user preferences.
    pub settings: AppSettings,
    /// Tool registry with safety policies.
    pub tools: ToolRegistry,
}

impl Inner {
    /// Recompute the effective `catalog` from the base + current custom models.
    pub fn rebuild_catalog(&mut self) {
        self.catalog = settings::merged_catalog(&self.base_catalog, &self.custom_models);
    }
}

pub struct AppState(pub Mutex<Inner>);

impl AppState {
    /// Build the initial state: open the store, load the bundled catalog, read
    /// any persisted consent + benchmark history. Never phones home.
    pub fn init() -> anyhow::Result<AppState> {
        let dir = data_dir();
        std::fs::create_dir_all(&dir).ok();
        let store = Store::open(&dir.join("fitllm.db"))?;
        let base_catalog = catalog::load_bundled()?;
        let custom_models = settings::load_custom_models(&dir);
        let catalog = settings::merged_catalog(&base_catalog, &custom_models);
        let settings = settings::load_settings(&dir);
        let benchmarks = store.all_benchmarks().unwrap_or_default();
        let consent_granted = std::fs::read_to_string(dir.join("consent")).is_ok();

        // Initialize tool system with safety limits
        let limits = ToolLimits::default();
        let destructive_allowed = consent_granted; // destructive tools require consent
        let mut tools = ToolRegistry::new(limits, destructive_allowed);
        tools.register(Box::new(ShellTool::new(ToolLimits::default())));
        tools.register(Box::new(FilesystemTool::new(ToolLimits::default())));
        tools.register(Box::new(ProcessTool::new(ToolLimits::default())));
        tools.register(Box::new(DesktopTool::new()));

        Ok(AppState(Mutex::new(Inner {
            store,
            catalog,
            base_catalog,
            custom_models,
            profile: None,
            benchmarks,
            consent_granted,
            settings,
            tools,
        })))
    }
}
