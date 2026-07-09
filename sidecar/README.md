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
