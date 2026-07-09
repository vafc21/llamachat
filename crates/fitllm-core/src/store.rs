//! SQLite local store. STUB — replaced by the store agent.

use crate::types::*;
use anyhow::Result;
use std::path::Path;

pub struct Store;

impl Store {
    pub fn open(_path: &Path) -> Result<Store> {
        Ok(Store)
    }
    pub fn open_in_memory() -> Result<Store> {
        Ok(Store)
    }
    pub fn save_profile(&self, _p: &HardwareProfile) -> Result<()> {
        Ok(())
    }
    pub fn latest_profile(&self) -> Result<Option<HardwareProfile>> {
        Ok(None)
    }
    pub fn save_benchmark(&self, _b: &BenchmarkResult) -> Result<()> {
        Ok(())
    }
    pub fn benchmarks_for(&self, _model: &str) -> Result<Vec<BenchmarkResult>> {
        Ok(vec![])
    }
    pub fn all_benchmarks(&self) -> Result<Vec<BenchmarkResult>> {
        Ok(vec![])
    }
}
