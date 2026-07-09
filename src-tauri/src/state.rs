//! Shared application state held behind a mutex and injected into every command.

use fitllm_core::{catalog, store::Store, BenchmarkResult, HardwareProfile, ModelCatalog};
use std::path::PathBuf;
use std::sync::Mutex;

/// Where FitLLM keeps its local SQLite store and settings.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("fitllm")
}

pub struct Inner {
    pub store: Store,
    pub catalog: ModelCatalog,
    pub profile: Option<HardwareProfile>,
    /// All benchmark results loaded/collected this session, newest last.
    pub benchmarks: Vec<BenchmarkResult>,
    pub consent_granted: bool,
}

pub struct AppState(pub Mutex<Inner>);

impl AppState {
    /// Build the initial state: open the store, load the bundled catalog, read
    /// any persisted consent + benchmark history. Never phones home.
    pub fn init() -> anyhow::Result<AppState> {
        let dir = data_dir();
        std::fs::create_dir_all(&dir).ok();
        let store = Store::open(&dir.join("fitllm.db"))?;
        let catalog = catalog::load_bundled()?;
        let benchmarks = store.all_benchmarks().unwrap_or_default();
        let consent_granted = std::fs::read_to_string(dir.join("consent")).is_ok();

        Ok(AppState(Mutex::new(Inner {
            store,
            catalog,
            profile: None,
            benchmarks,
            consent_granted,
        })))
    }
}
