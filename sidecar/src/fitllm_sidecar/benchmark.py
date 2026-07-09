"""Benchmark orchestration.

Thin, defensive wrapper around a :class:`RuntimeAdapter`'s ``run_benchmark``.
Handles the "backend not running" and "system too busy" cases up front and
always returns a well-formed ``BenchmarkResult`` dict — it never raises.
"""

from __future__ import annotations

from typing import Callable, Optional

from .adapters.base import RuntimeAdapter, empty_result
from .sysmon import cpu_load

# Refuse to start a benchmark when background CPU load is above this fraction —
# the numbers would be meaningless. Only enforced when psutil is available.
MAX_START_LOAD = 0.8


def run_benchmark(
    adapter: RuntimeAdapter,
    model: str,
    tier: str = "quick",
    progress: Optional[Callable[[str, int], None]] = None,
) -> dict:
    """Run a benchmark for ``model`` on ``adapter`` and return a result dict.

    ``progress`` is an optional ``callback(stage, pct)``.
    """
    tier = tier if tier in ("quick", "full") else "quick"

    if progress:
        progress("checking adapter", 2)

    # 1. Backend reachable?
    try:
        available = adapter.is_available()
    except Exception as exc:  # never crash on a flaky availability probe
        return empty_result(model, adapter.name, tier, f"availability check failed: {exc}")

    if not available:
        detail = (
            f"Ollama not available at {getattr(adapter, 'base_url', 'http://127.0.0.1:11434')}"
            if adapter.name == "ollama"
            else f"adapter '{adapter.name}' not available"
        )
        return empty_result(model, adapter.name, tier, detail)

    # 2. Resource capping — don't benchmark on an already-saturated machine.
    load = cpu_load(interval=0.5)  # None if psutil missing -> skip the cap
    if load is not None and load > MAX_START_LOAD:
        return empty_result(
            model,
            adapter.name,
            tier,
            f"system too busy to benchmark (background load {load:.2f} > {MAX_START_LOAD})",
            background_load=load,
        )

    if progress:
        progress("running benchmark", 8)

    # 3. Delegate. Pass progress through when the adapter supports it.
    try:
        try:
            result = adapter.run_benchmark(model, None, tier, progress=progress)  # type: ignore[call-arg]
        except TypeError:
            # Adapter doesn't accept a progress kwarg — call the ABC signature.
            result = adapter.run_benchmark(model, None, tier)
    except Exception as exc:
        return empty_result(model, adapter.name, tier, f"benchmark error: {exc}")

    if progress:
        progress("done", 100)
    return result
