//! SQLite local store for hardware profiles and benchmark history.
//!
//! Backed by `rusqlite` (bundled SQLite, so no system library needed). Both the
//! profile and the benchmark rows persist the canonical type as a JSON blob
//! alongside a few indexed columns, so the schema stays forward-compatible with
//! changes to the shared types without a migration for every field.

use crate::types::*;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

/// Current schema version; bump when the table shapes change.
const SCHEMA_VERSION: i64 = 1;

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (creating if needed) a store at `path`.
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening SQLite database at {}", path.display()))?;
        Self::init(conn)
    }

    /// Open an ephemeral in-memory store (used by tests and dry runs).
    pub fn open_in_memory() -> Result<Store> {
        let conn = Connection::open_in_memory().context("opening in-memory SQLite database")?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS hardware_profiles (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                captured_at TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                json        TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS benchmark_results (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                model      TEXT NOT NULL,
                adapter    TEXT NOT NULL,
                ok         INTEGER NOT NULL,
                gen_tps    REAL,
                tier       TEXT NOT NULL,
                created_at TEXT NOT NULL,
                json       TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_benchmarks_model
                ON benchmark_results (model);
            "#,
        )
        .context("creating schema")?;

        conn.execute(
            "INSERT OR IGNORE INTO schema_meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )
        .context("recording schema version")?;

        Ok(Store { conn })
    }

    /// Persist a hardware profile snapshot.
    pub fn save_profile(&self, p: &HardwareProfile) -> Result<()> {
        let json = serde_json::to_string(p).context("serializing HardwareProfile")?;
        let captured_at = if p.detected_at.is_empty() {
            now_iso()
        } else {
            p.detected_at.clone()
        };
        self.conn
            .execute(
                "INSERT INTO hardware_profiles (captured_at, created_at, json) VALUES (?1, ?2, ?3)",
                params![captured_at, now_iso(), json],
            )
            .context("inserting hardware profile")?;
        Ok(())
    }

    /// Return the most recently inserted profile, if any.
    pub fn latest_profile(&self) -> Result<Option<HardwareProfile>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM hardware_profiles ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional_row()?;
        match json {
            Some(j) => Ok(Some(
                serde_json::from_str(&j).context("deserializing HardwareProfile")?,
            )),
            None => Ok(None),
        }
    }

    /// Persist one benchmark result.
    pub fn save_benchmark(&self, b: &BenchmarkResult) -> Result<()> {
        let json = serde_json::to_string(b).context("serializing BenchmarkResult")?;
        let created_at = if b.timestamp.is_empty() {
            now_iso()
        } else {
            b.timestamp.clone()
        };
        self.conn
            .execute(
                "INSERT INTO benchmark_results
                    (model, adapter, ok, gen_tps, tier, created_at, json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    b.model,
                    b.adapter,
                    b.ok as i64,
                    b.gen_tps,
                    b.tier,
                    created_at,
                    json
                ],
            )
            .context("inserting benchmark result")?;
        Ok(())
    }

    /// All benchmarks recorded for a given model tag, newest first.
    pub fn benchmarks_for(&self, model: &str) -> Result<Vec<BenchmarkResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT json FROM benchmark_results WHERE model = ?1 ORDER BY id DESC",
        )?;
        let rows = stmt
            .query_map(params![model], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        parse_benchmarks(rows)
    }

    /// Every benchmark in the store, newest first.
    pub fn all_benchmarks(&self) -> Result<Vec<BenchmarkResult>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM benchmark_results ORDER BY id DESC")?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        parse_benchmarks(rows)
    }
}

fn parse_benchmarks(rows: Vec<String>) -> Result<Vec<BenchmarkResult>> {
    rows.into_iter()
        .map(|j| serde_json::from_str(&j).context("deserializing BenchmarkResult"))
        .collect()
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Small helper: turn a `QueryReturnedNoRows` error into `Ok(None)`.
trait OptionalRow<T> {
    fn optional_row(self) -> Result<Option<T>>;
}

impl<T> OptionalRow<T> for rusqlite::Result<T> {
    fn optional_row(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).context("querying store"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> HardwareProfile {
        HardwareProfile {
            cpu: Cpu {
                model: "Ryzen".into(),
                vendor: "AMD".into(),
                physical_cores: 8,
                logical_cores: 16,
                base_clock_mhz: None,
                max_clock_mhz: None,
                flags: CpuFlags::default(),
            },
            gpus: vec![],
            apple_silicon: None,
            memory: Memory {
                total_mb: 32_000,
                available_mb: 24_000,
            },
            storage: Storage {
                models_dir: "/models".into(),
                free_mb: 100_000,
                read_mbps: None,
            },
            os: Os {
                name: "linux".into(),
                version: "6".into(),
                arch: "x86_64".into(),
            },
            backends: vec!["cpu".into()],
            detected_at: "2026-07-09T04:00:00Z".into(),
        }
    }

    fn sample_bench(model: &str, ts: &str, tps: f64) -> BenchmarkResult {
        BenchmarkResult {
            model: model.into(),
            adapter: "ollama".into(),
            ok: true,
            error: None,
            prompt_eval_tps: Some(800.0),
            gen_tps: Some(tps),
            ttft_ms: Some(200.0),
            peak_mem_mb: Some(4200.0),
            context_tested: 512,
            background_load: Some(0.1),
            tier: "quick".into(),
            timestamp: ts.into(),
        }
    }

    #[test]
    fn profile_roundtrip_returns_latest() {
        let s = Store::open_in_memory().unwrap();
        assert!(s.latest_profile().unwrap().is_none());

        let mut p1 = sample_profile();
        p1.memory.total_mb = 16_000;
        s.save_profile(&p1).unwrap();

        let mut p2 = sample_profile();
        p2.memory.total_mb = 64_000;
        s.save_profile(&p2).unwrap();

        let latest = s.latest_profile().unwrap().unwrap();
        assert_eq!(latest.memory.total_mb, 64_000);
    }

    #[test]
    fn benchmark_query_by_model_and_all() {
        let s = Store::open_in_memory().unwrap();
        s.save_benchmark(&sample_bench("llama3.1:8b", "2026-07-09T04:10:00Z", 40.0))
            .unwrap();
        s.save_benchmark(&sample_bench("llama3.1:8b", "2026-07-09T04:20:00Z", 42.0))
            .unwrap();
        s.save_benchmark(&sample_bench("mistral:7b", "2026-07-09T04:15:00Z", 55.0))
            .unwrap();

        let l = s.benchmarks_for("llama3.1:8b").unwrap();
        assert_eq!(l.len(), 2);
        // Newest first.
        assert_eq!(l[0].gen_tps, Some(42.0));

        let all = s.all_benchmarks().unwrap();
        assert_eq!(all.len(), 3);

        assert_eq!(s.benchmarks_for("does-not-exist").unwrap().len(), 0);
    }

    #[test]
    fn persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("llamachat.db");
        {
            let s = Store::open(&path).unwrap();
            s.save_profile(&sample_profile()).unwrap();
        }
        // Reopen: data survived.
        let s2 = Store::open(&path).unwrap();
        assert!(s2.latest_profile().unwrap().is_some());
    }
}
