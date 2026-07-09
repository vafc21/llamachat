//! Model catalog loader. STUB — replaced by the catalog agent.

use crate::types::ModelCatalog;
use anyhow::Result;

pub fn load_bundled() -> Result<ModelCatalog> {
    load_from_str(r#"{"schema_version":1,"updated_at":"","models":[],"frontier":[]}"#)
}

pub fn load_from_str(s: &str) -> Result<ModelCatalog> {
    Ok(serde_json::from_str(s)?)
}
