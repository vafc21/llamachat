//! FitLLM core engine.
//!
//! Pure-Rust library with no GUI dependencies, so it builds and runs on any
//! host (including CI and machines without webkit2gtk). The Tauri shell and the
//! CLI both depend on this crate.
//!
//! Public surface:
//! - [`hardware::profile`] — capture a [`HardwareProfile`] of this machine.
//! - [`catalog::load_bundled`] / [`catalog::load_from_str`] — the model catalog.
//! - [`recommend::rate_all`] — turn profile + catalog (+ benchmarks) into
//!   [`Recommendation`]s.
//! - [`store::Store`] — SQLite persistence for profiles and benchmark history.

pub mod catalog;
pub mod hardware;
pub mod recommend;
pub mod store;
pub mod types;

pub use types::*;
