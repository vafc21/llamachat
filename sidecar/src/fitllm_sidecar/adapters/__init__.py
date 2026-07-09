"""Runtime adapters for FitLLM.

Phase 1 ships a single concrete adapter (:class:`OllamaAdapter`) plus the
:class:`RuntimeAdapter` ABC. Register new backends in ``ADAPTERS``.
"""

from __future__ import annotations

from typing import Optional

from .base import RuntimeAdapter
from .ollama import OllamaAdapter

# name -> factory. Adding a backend is "one file" + one line here.
ADAPTERS = {
    "ollama": OllamaAdapter,
}


def get_adapter(name: str) -> Optional[RuntimeAdapter]:
    """Instantiate the adapter registered under ``name`` (or ``None``)."""
    factory = ADAPTERS.get(name)
    return factory() if factory else None


def list_adapters() -> list[dict]:
    """Describe every registered adapter as ``{name, available, detail}``."""
    out = []
    for name, factory in ADAPTERS.items():
        adapter = factory()
        try:
            available = adapter.is_available()
        except Exception:
            available = False
        detail = (
            f"Ollama HTTP API at {getattr(adapter, 'base_url', '')}"
            if name == "ollama"
            else name
        )
        if not available and name == "ollama":
            detail += " (not reachable)"
        out.append({"name": name, "available": available, "detail": detail})
    return out


__all__ = [
    "RuntimeAdapter",
    "OllamaAdapter",
    "ADAPTERS",
    "get_adapter",
    "list_adapters",
]
