# FitLLM — Internal Interface Contract (Phase 1)

This file pins the interfaces between the parallel-built modules. **Do not
change a shared type or signature without updating this file.** The canonical
Rust definitions live in `crates/fitllm-core/src/types.rs`.

## Repo layout

```
fitllm/
  Cargo.toml                 # workspace; default-members = core + cli (build w/o webkit)
  crates/
    fitllm-core/             # pure-Rust lib: types, hardware, catalog, recommend, store
      src/
        types.rs             # SHARED TYPES — owned by lead, do not edit signatures
        lib.rs               # module decls — owned by lead
        hardware/            # OWNER: hardware agent
        catalog.rs           # OWNER: catalog/recommend agent
        recommend.rs         # OWNER: catalog/recommend agent
        store.rs             # OWNER: catalog/recommend agent
    fitllm-cli/              # OWNER: catalog/recommend agent (bin that ties core together)
  catalog/models.json        # OWNER: catalog/recommend agent (bundled model data)
  sidecar/                   # OWNER: sidecar agent (Python benchmark orchestration)
  ui/                        # OWNER: ui agent (React + Tailwind + Vite)
  src-tauri/                 # OWNER: lead (Tauri shell wiring)
```

## Rust core public API (what CLI + Tauri call)

```rust
// hardware
fn hardware::profile() -> anyhow::Result<HardwareProfile>;

// catalog
fn catalog::load_bundled() -> anyhow::Result<ModelCatalog>;   // embeds catalog/models.json via include_str!
fn catalog::load_from_str(s: &str) -> anyhow::Result<ModelCatalog>;

// recommend
fn recommend::rate_all(
    profile: &HardwareProfile,
    catalog: &ModelCatalog,
    benchmarks: &[BenchmarkResult],   // may be empty -> heuristic ratings only
) -> Vec<Recommendation>;             // sorted best-first

// store (SQLite)
struct store::Store;
impl Store {
    fn open(path: &std::path::Path) -> anyhow::Result<Store>;
    fn open_in_memory() -> anyhow::Result<Store>;
    fn save_profile(&self, p: &HardwareProfile) -> anyhow::Result<()>;
    fn latest_profile(&self) -> anyhow::Result<Option<HardwareProfile>>;
    fn save_benchmark(&self, b: &BenchmarkResult) -> anyhow::Result<()>;
    fn benchmarks_for(&self, model: &str) -> anyhow::Result<Vec<BenchmarkResult>>;
    fn all_benchmarks(&self) -> anyhow::Result<Vec<BenchmarkResult>>;
}
```

All shared struct/enum shapes are in `types.rs`. Their JSON serialization is the
contract with the Python sidecar and the UI.

## Python sidecar protocol

The sidecar is a Python package under `sidecar/`. It runs two ways:

1. **CLI (for testing / standalone):**
   - `python -m fitllm_sidecar list-adapters` → prints `{"adapters":[{"name","available","detail"}]}`
   - `python -m fitllm_sidecar list-models --adapter ollama` → `{"models":[{"name","size_mb"}]}`
   - `python -m fitllm_sidecar benchmark --adapter ollama --model <tag> [--tier quick|full]`
     → prints one `BenchmarkResult` JSON object (see types.rs shape).

2. **Serve mode (spawned by Tauri as a sidecar):** `python -m fitllm_sidecar serve`
   - Reads newline-delimited JSON requests on stdin, writes newline-delimited
     JSON responses on stdout.
   - Request: `{"id": <int>, "method": <str>, "params": {...}}`
   - Methods: `list_adapters`, `list_models`, `quick_benchmark`, `ping`.
   - `quick_benchmark` params: `{"adapter":"ollama","model":"<tag>"}`.
   - Response: `{"id": <int>, "result": {...}}` or `{"id": <int>, "error": "<msg>"}`.
   - Progress (optional, out of band): `{"event":"progress","stage":<str>,"pct":<0-100>,"model":<str>}`.

`BenchmarkResult` JSON the sidecar emits (matches `types.rs`):
```json
{
  "model": "llama3.2:3b", "adapter": "ollama", "ok": true, "error": null,
  "prompt_eval_tps": 850.0, "gen_tps": 62.3, "ttft_ms": 240.0,
  "peak_mem_mb": 4200.0, "context_tested": 512, "background_load": 0.15,
  "tier": "quick", "timestamp": "2026-07-09T04:40:00Z"
}
```

Adapter interface (Python, so new backends are one file):
```python
class RuntimeAdapter:
    name: str
    def is_available(self) -> bool: ...
    def list_models(self) -> list[dict]: ...           # [{"name","size_mb"}]
    def pull(self, model: str) -> Iterator[dict]: ...   # progress dicts
    def run_benchmark(self, model, prompts, tier) -> BenchmarkResult(dict): ...
    def stream_generate(self, model, prompt) -> Iterator[str]: ...
```
Phase 1 ships `OllamaAdapter` only, plus the ABC. Ollama HTTP API at
`http://127.0.0.1:11434`. Degrade gracefully if Ollama is not running:
`is_available()` → False, benchmark → `{"ok": false, "error": "..."}`.

## UI ↔ core (Tauri IPC command names)

The UI calls these Tauri commands (the lead wires them; UI agent uses a mock
data layer with the same shapes so it runs standalone via `npm run dev`):
- `get_hardware_profile()` → `HardwareProfile`
- `get_recommendations()` → `Recommendation[]` (heuristic first, then measured)
- `start_quick_benchmark()` → kicks off background benchmark; emits `benchmark_progress` and `recommendations_updated` events
- `get_catalog()` → `ModelCatalog`
- `set_consent(granted: bool)` / `get_consent()` → onboarding consent state
- `export_data()` / `wipe_data()` → privacy controls

The UI must have a typed mock layer (`ui/src/lib/api.ts`) so it renders with
sample data when not running inside Tauri (`window.__TAURI__` absent).
