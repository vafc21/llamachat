"""System-load / memory sampling helpers built on ``psutil``.

Every function here degrades gracefully: if ``psutil`` is not importable, the
load/memory samplers return ``None`` instead of raising, so the rest of the
sidecar can keep running and simply report those benchmark fields as null.
"""

from __future__ import annotations

from typing import Optional

try:  # psutil is optional at runtime — never let its absence crash the sidecar.
    import psutil  # type: ignore

    HAVE_PSUTIL = True
except Exception:  # pragma: no cover - environment dependent
    psutil = None  # type: ignore
    HAVE_PSUTIL = False


def cpu_load(interval: float = 0.5) -> Optional[float]:
    """Current CPU utilization as a 0.0-1.0 fraction, or ``None`` if unknown.

    Blocks for ``interval`` seconds to get a meaningful (non-zero-on-first-call)
    sample from psutil.
    """
    if not HAVE_PSUTIL:
        return None
    try:
        return float(psutil.cpu_percent(interval=interval)) / 100.0
    except Exception:
        return None


def mem_used_mb() -> Optional[float]:
    """System RAM currently in use, in MB, or ``None`` if unknown."""
    if not HAVE_PSUTIL:
        return None
    try:
        return float(psutil.virtual_memory().used) / (1024.0 * 1024.0)
    except Exception:
        return None
