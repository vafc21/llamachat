"""RuntimeAdapter abstract base class.

A ``RuntimeAdapter`` is a thin wrapper around one local inference backend (e.g.
Ollama). Phase 1 ships a single concrete adapter (``OllamaAdapter``); adding a
new backend is meant to be "one file" — subclass this and implement the five
methods below.

The dict returned by :meth:`run_benchmark` MUST match the ``BenchmarkResult``
shape defined in ``crates/fitllm-core/src/types.rs``::

    {
      "model": str, "adapter": str, "ok": bool, "error": Optional[str],
      "prompt_eval_tps": Optional[float], "gen_tps": Optional[float],
      "ttft_ms": Optional[float], "peak_mem_mb": Optional[float],
      "context_tested": int, "background_load": Optional[float],
      "tier": str, "timestamp": str
    }
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from datetime import datetime, timezone
from typing import Iterator, Optional


def utc_now_iso() -> str:
    """ISO-8601 UTC timestamp, e.g. ``2026-07-09T04:40:00Z`` (contract shape)."""
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def empty_result(
    model: str,
    adapter: str,
    tier: str,
    error: str,
    *,
    context_tested: int = 0,
    background_load: Optional[float] = None,
) -> dict:
    """Build a well-formed, failed ``BenchmarkResult`` dict.

    Used everywhere we need to fail gracefully without crashing.
    """
    return {
        "model": model,
        "adapter": adapter,
        "ok": False,
        "error": error,
        "prompt_eval_tps": None,
        "gen_tps": None,
        "ttft_ms": None,
        "peak_mem_mb": None,
        "context_tested": context_tested,
        "background_load": background_load,
        "tier": tier,
        "timestamp": utc_now_iso(),
    }


class RuntimeAdapter(ABC):
    """Abstract interface every inference backend implements."""

    @property
    @abstractmethod
    def name(self) -> str:
        """Stable adapter id, e.g. ``"ollama"``."""
        raise NotImplementedError

    @abstractmethod
    def is_available(self) -> bool:
        """True if the backend is reachable/usable right now."""
        raise NotImplementedError

    @abstractmethod
    def list_models(self) -> list[dict]:
        """Locally available models as ``[{"name", "size_mb"}, ...]``."""
        raise NotImplementedError

    @abstractmethod
    def pull(self, model: str) -> Iterator[dict]:
        """Download ``model``, yielding progress dicts as they arrive."""
        raise NotImplementedError

    @abstractmethod
    def run_benchmark(self, model: str, prompts, tier: str) -> dict:
        """Benchmark ``model`` and return a ``BenchmarkResult`` dict.

        ``prompts`` may be ``None``, in which case the adapter supplies a
        default set appropriate for ``tier`` ("quick" | "balanced" | "full").
        """
        raise NotImplementedError

    @abstractmethod
    def stream_generate(self, model: str, prompt: str) -> Iterator[str]:
        """Generate a completion, yielding token/text chunks as they arrive."""
        raise NotImplementedError
