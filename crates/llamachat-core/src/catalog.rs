//! Model catalog loader.
//!
//! The bundled catalog lives at `catalog/models.json` in the repo root and is
//! embedded into the binary at compile time via [`include_str!`]. Callers that
//! want to hot-load an updated catalog from disk can use [`load_from_str`].

use crate::types::ModelCatalog;
use anyhow::{Context, Result};

/// The raw JSON of the bundled catalog, embedded at compile time.
///
/// The path is resolved relative to *this source file* (`crates/llamachat-core/src/`),
/// so three `..` hops reach the workspace root where `catalog/` lives.
const BUNDLED_JSON: &str = include_str!("../../../catalog/models.json");

/// Load the catalog that ships with the binary.
pub fn load_bundled() -> Result<ModelCatalog> {
    load_from_str(BUNDLED_JSON).context("parsing bundled catalog/models.json")
}

/// Parse a catalog from a JSON string (e.g. a user-updated catalog on disk).
pub fn load_from_str(s: &str) -> Result<ModelCatalog> {
    let catalog: ModelCatalog =
        serde_json::from_str(s).context("deserializing ModelCatalog from JSON")?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_loads_and_is_sane() {
        let cat = load_bundled().expect("bundled catalog should parse");
        assert_eq!(cat.schema_version, 1);
        assert!(cat.models.len() >= 12, "expected >= 12 models");
        assert!(cat.frontier.len() >= 3, "expected frontier references");

        for m in &cat.models {
            assert!(!m.id.is_empty(), "model id must be set");
            assert!(m.params_b > 0.0, "{} params_b", m.id);
            assert!(
                m.quality_score >= 0.0 && m.quality_score <= 100.0,
                "{} quality_score out of range",
                m.id
            );
            assert!(m.context_max >= m.context_default, "{} context", m.id);
            // Every model must carry at least Q4_K_M and Q8_0.
            let names: Vec<&str> = m.quants.iter().map(|q| q.name.as_str()).collect();
            assert!(names.contains(&"Q4_K_M"), "{} missing Q4_K_M", m.id);
            assert!(names.contains(&"Q8_0"), "{} missing Q8_0", m.id);
            for q in &m.quants {
                assert!(q.size_mb > 0, "{} {} size_mb", m.id, q.name);
            }
        }

        // Size classes from ~1B to ~70B are represented.
        let min = cat.models.iter().map(|m| m.params_b).fold(f64::MAX, f64::min);
        let max = cat.models.iter().map(|m| m.params_b).fold(0.0_f64, f64::max);
        assert!(min <= 2.0, "smallest model should be ~1B, got {min}");
        assert!(max >= 65.0, "largest model should be ~70B, got {max}");
    }
}
