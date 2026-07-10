# fitllm-sidecar

Python benchmark sidecar for **FitLLM**. It orchestrates on-device LLM
benchmarks and talks to runtime backends (Phase 1: **Ollama**) over HTTP, then
emits `BenchmarkResult` JSON whose shape matches
`crates/fitllm-core/src/types.rs` so the Rust core can deserialize it directly.

## Running

The package lives under `src/`. With no install step, run it by pointing
`PYTHONPATH` at `src/`:

```bash
# from the sidecar/ directory
PYTHONPATH=src python -m fitllm_sidecar list-adapters
PYTHONPATH=src python -m fitllm_sidecar list-models --adapter ollama
PYTHONPATH=src python -m fitllm_sidecar benchmark --adapter ollama --model llama3.2:1b --tier quick
PYTHONPATH=src python -m fitllm_sidecar serve
```

Or install it (`pip install -e .`) to get the `fitllm-sidecar` console script.

## Benchmark intensity tiers

`benchmark --tier {quick,balanced,full}` selects how deep the measurement goes.
Every tier emits the **same** `BenchmarkResult` JSON shape (see
`../CONTRACT.md`); only the depth of measurement — and the `tier` field —
changes. Reported `gen_tps` / `prompt_eval_tps` / `ttft_ms` are averaged across
all samples, so higher tiers yield more stable numbers.

| Tier       | Prompts            | num_predict | Context windows (num_ctx) | Repeats | Warmup | Budget        |
|------------|--------------------|-------------|---------------------------|---------|--------|---------------|
| `quick`    | 3 short            | 100         | 512 (not forced)          | 1       | 0      | seconds       |
| `balanced` | 4 medium           | 200         | 512, 2048                 | 2       | 1      | a few minutes |
| `full`     | 5 long             | 500         | 512, 2048, 4096           | 3       | 1      | highest cost  |

- **quick** is byte-for-byte the historical behavior: one pass over the short
  prompt set, no `num_ctx` override on the request, no warmup, no repeats.
- **balanced / full** run every prompt at every context window, `repeats`
  times, and average the samples. A `warmup` generation runs first (its cost is
  discarded) to absorb model-load / cache warm-up.
- `context_tested` in the result reports the **largest** context window that
  produced a successful sample (so a model that can't fit 4096 still reports the
  largest window it actually managed).

Tier config lives in `TIER_CONFIG` in
`src/fitllm_sidecar/adapters/ollama.py`.

## Packaging (frozen single-file binary)

To ship a self-contained sidecar in the macOS `.dmg` (and other bundles) with
**no system Python required**, freeze it with PyInstaller into a binary named
`fitllm-sidecar`.

```bash
# from the repo root
python3 -m venv .venv-build && . .venv-build/bin/activate
pip install -r scripts/requirements-sidecar.txt   # pins requests, psutil, pyinstaller
scripts/build-sidecar.sh
```

This produces:

- `dist/fitllm-sidecar` — the raw PyInstaller onefile binary.
- `src-tauri/binaries/fitllm-sidecar-<target-triple>` — staged for Tauri.

Tauri v2 bundles it as an **external binary**. `src-tauri/tauri.conf.json`
declares:

```json
"bundle": { "externalBin": ["binaries/fitllm-sidecar"] }
```

On disk the file must carry the Rust target-triple suffix
(e.g. `fitllm-sidecar-aarch64-apple-darwin`); the build script adds it
automatically from `rustc -vV` (override with `FITLLM_SIDECAR_TRIPLE`). When the
app is bundled, the triple suffix is stripped and the binary lands **next to the
app executable** (on macOS: `FitLLM.app/Contents/MacOS/fitllm-sidecar`).

The frozen binary takes the same argv as `python -m fitllm_sidecar`, e.g.:

```bash
./fitllm-sidecar benchmark --adapter ollama --model llama3.2:1b --tier full
./fitllm-sidecar serve
```

Relevant files:

- `scripts/fitllm-sidecar.spec` — PyInstaller spec (entry, hidden imports).
- `scripts/fitllm_sidecar_entry.py` — launcher that runs `fitllm_sidecar.__main__:main`.
- `scripts/build-sidecar.sh` — freeze + stage script.
- `scripts/requirements-sidecar.txt` — pinned build/runtime deps.

## Dependencies

- `requests` — required for the Ollama HTTP API.
- `psutil` — used for background CPU load and memory sampling. **Optional at
  runtime**: if it is not importable, `background_load` and `peak_mem_mb` are
  reported as `null` and the sidecar keeps working.

## Serve protocol

Newline-delimited JSON on stdin/stdout:

- Request:  `{"id": <int>, "method": <str>, "params": {...}}`
- Response: `{"id": <int>, "result": {...}}` or `{"id": <int>, "error": "<msg>"}`
- Progress (out of band): `{"event": "progress", "stage": <str>, "pct": <0-100>, "model": <str>}`

Methods: `ping`, `list_adapters`, `list_models`, `quick_benchmark`.
