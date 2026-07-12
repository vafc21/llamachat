"""LlamaChat Python benchmark sidecar.

Orchestrates on-device LLM benchmarks and talks to runtime backends (Phase 1:
Ollama) over HTTP. Emits ``BenchmarkResult`` JSON objects whose shape matches
``crates/fitllm-core/src/types.rs`` so the Rust core can deserialize them.

Requires ``requests``. ``psutil`` is used when available for background-load and
memory sampling, but is imported defensively — the sidecar degrades gracefully
(reporting ``null`` for those fields) if it is missing.
"""

from .adapters.base import RuntimeAdapter
from .adapters.ollama import OllamaAdapter
from .benchmark import run_benchmark
from .server import serve

__version__ = "0.1.0"

__all__ = [
    "RuntimeAdapter",
    "OllamaAdapter",
    "run_benchmark",
    "serve",
    "__version__",
]
