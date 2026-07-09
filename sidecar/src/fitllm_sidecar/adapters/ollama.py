"""Ollama runtime adapter.

Talks to a local Ollama server over its HTTP API at
``http://127.0.0.1:11434``. Requires ``requests``; uses ``psutil`` when
available (via :mod:`fitllm_sidecar.sysmon`) for background-load and memory
sampling, degrading to ``None`` for those fields otherwise.
"""

from __future__ import annotations

import json
from typing import Callable, Iterator, Optional

from .base import RuntimeAdapter, empty_result, utc_now_iso
from ..sysmon import cpu_load, mem_used_mb

try:
    import requests  # type: ignore

    HAVE_REQUESTS = True
except Exception:  # pragma: no cover - requests is a hard dep, but be defensive
    requests = None  # type: ignore
    HAVE_REQUESTS = False

OLLAMA_URL = "http://127.0.0.1:11434"

# Lightweight calls (tags / pull status framing) use a 30s timeout. Generation
# can legitimately run longer than 30s (e.g. 500 tokens on CPU), so it uses a
# 30s *connect* timeout with a generous *read* timeout so a benchmark is not
# aborted mid-generation.
HTTP_TIMEOUT = 30
GEN_TIMEOUT = (30, 600)

# Prompt sets by tier.
QUICK_PROMPTS = [
    "In one sentence, what is a large language model?",
    "List three primary colors.",
    "Write a haiku about the ocean.",
]
FULL_PROMPTS = [
    "Explain how a transformer neural network works, covering attention, "
    "positional encoding, and the feed-forward layers.",
    "Write a short story (a few paragraphs) about a lighthouse keeper who "
    "discovers a message in a bottle.",
    "Describe the trade-offs between quantization levels (Q4, Q8, FP16) for "
    "running language models locally on consumer hardware.",
    "Summarize the history of computing from the 1940s to today in several "
    "well-organized paragraphs.",
    "Explain, step by step, how you would design a REST API for a to-do list "
    "application, including endpoints, data models, and error handling.",
]

# Output-token caps and a representative context size per tier.
TIER_MAX_TOKENS = {"quick": 100, "full": 500}
TIER_CONTEXT = {"quick": 512, "full": 2048}


class OllamaAdapter(RuntimeAdapter):
    """Concrete :class:`RuntimeAdapter` for a local Ollama server."""

    def __init__(self, base_url: str = OLLAMA_URL):
        self.base_url = base_url.rstrip("/")

    @property
    def name(self) -> str:
        return "ollama"

    # -- availability / discovery ------------------------------------------

    def is_available(self) -> bool:
        if not HAVE_REQUESTS:
            return False
        try:
            resp = requests.get(f"{self.base_url}/api/tags", timeout=HTTP_TIMEOUT)
            return resp.status_code == 200
        except Exception:
            return False

    def list_models(self) -> list[dict]:
        if not HAVE_REQUESTS:
            return []
        try:
            resp = requests.get(f"{self.base_url}/api/tags", timeout=HTTP_TIMEOUT)
            resp.raise_for_status()
            data = resp.json()
        except Exception:
            return []
        models = []
        for m in data.get("models", []) or []:
            size = m.get("size") or 0
            models.append(
                {"name": m.get("name", ""), "size_mb": int(size) // (1024 * 1024)}
            )
        return models

    def _model_size_mb(self, model: str) -> Optional[float]:
        """Best-effort on-disk size (MB) of ``model`` from /api/tags."""
        for m in self.list_models():
            if m["name"] == model:
                return float(m["size_mb"])
        return None

    # -- generation --------------------------------------------------------

    def stream_generate(self, model: str, prompt: str) -> Iterator[str]:
        if not HAVE_REQUESTS:
            return
        try:
            resp = requests.post(
                f"{self.base_url}/api/generate",
                json={"model": model, "prompt": prompt, "stream": True},
                stream=True,
                timeout=GEN_TIMEOUT,
            )
            resp.raise_for_status()
            for line in resp.iter_lines(decode_unicode=True):
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except Exception:
                    continue
                chunk = obj.get("response")
                if chunk:
                    yield chunk
                if obj.get("done"):
                    break
        except Exception:
            return

    def chat(
        self,
        model: str,
        messages: list[dict],
        system: str = "",
        stream: bool = True,
    ) -> Iterator[str]:
        """Send a conversation to Ollama /api/chat and yield response tokens.

        ``messages`` is a list of {role, content} dicts (roles: user/assistant/system).
        ``system`` is an optional system prompt prepended to the messages.
        When ``stream=True``, yields individual token strings. When ``stream=False``,
        yields a single string with the complete response.
        """
        if not HAVE_REQUESTS:
            return

        # Build the Ollama chat payload
        msgs: list[dict] = []
        if system:
            msgs.append({"role": "system", "content": system})

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")
            msgs.append({"role": role, "content": content})

        try:
            resp = requests.post(
                f"{self.base_url}/api/chat",
                json={
                    "model": model,
                    "messages": msgs,
                    "stream": stream,
                },
                stream=stream,
                timeout=GEN_TIMEOUT,
            )
            resp.raise_for_status()

            if not stream:
                obj = resp.json()
                content = obj.get("message", {}).get("content", "")
                if content:
                    yield content
                return

            for line in resp.iter_lines(decode_unicode=True):
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except Exception:
                    continue
                chunk = obj.get("message", {}).get("content", "")
                if chunk:
                    yield chunk
                if obj.get("done"):
                    break
        except Exception:
            return

    def pull(self, model: str) -> Iterator[dict]:
        if not HAVE_REQUESTS:
            yield {"status": "error", "error": "requests not available"}
            return
        try:
            resp = requests.post(
                f"{self.base_url}/api/pull",
                json={"model": model, "stream": True},
                stream=True,
                timeout=GEN_TIMEOUT,
            )
            resp.raise_for_status()
            for line in resp.iter_lines(decode_unicode=True):
                if not line:
                    continue
                try:
                    yield json.loads(line)
                except Exception:
                    continue
        except Exception as exc:
            yield {"status": "error", "error": str(exc)}

    # -- benchmarking ------------------------------------------------------

    def run_benchmark(
        self,
        model: str,
        prompts,
        tier: str,
        progress: Optional[Callable[[str, int], None]] = None,
    ) -> dict:
        """Benchmark ``model`` on ``prompts`` and return a BenchmarkResult dict.

        ``progress`` is an optional ``callback(stage, pct)`` used by serve-mode
        to emit out-of-band progress events.
        """
        tier = tier if tier in ("quick", "full") else "quick"
        if prompts is None:
            prompts = QUICK_PROMPTS if tier == "quick" else FULL_PROMPTS
        max_tokens = TIER_MAX_TOKENS[tier]
        context_tested = TIER_CONTEXT[tier]

        if not HAVE_REQUESTS:
            return empty_result(
                model, self.name, tier, "requests library not available"
            )
        if not self.is_available():
            return empty_result(
                model,
                self.name,
                tier,
                f"Ollama not available at {self.base_url}",
                context_tested=context_tested,
            )

        model_size_mb = self._model_size_mb(model)

        prompt_tps_samples: list[float] = []
        gen_tps_samples: list[float] = []
        ttft_samples: list[float] = []
        load_samples: list[float] = []
        peak_mem_delta: Optional[float] = None
        succeeded = 0
        last_error: Optional[str] = None

        total = len(prompts)
        for idx, prompt in enumerate(prompts):
            if progress:
                pct = int(round(10 + (idx / max(total, 1)) * 85))
                progress(f"benchmark {idx + 1}/{total}", pct)

            # 1. Background load just before the request (0.0-1.0 or None).
            load = cpu_load(interval=0.5)
            if load is not None:
                load_samples.append(load)

            mem_before = mem_used_mb()
            try:
                resp = requests.post(
                    f"{self.base_url}/api/generate",
                    json={
                        "model": model,
                        "prompt": prompt,
                        "stream": False,
                        "options": {"num_predict": max_tokens},
                    },
                    timeout=GEN_TIMEOUT,
                )
                resp.raise_for_status()
                obj = resp.json()
            except Exception as exc:
                last_error = str(exc)
                continue

            if obj.get("error"):
                last_error = str(obj["error"])
                continue

            mem_after = mem_used_mb()
            if mem_before is not None and mem_after is not None:
                delta = mem_after - mem_before
                if delta > 0:
                    peak_mem_delta = (
                        delta if peak_mem_delta is None else max(peak_mem_delta, delta)
                    )

            # 3./4. Parse timings and compute throughput.
            eval_count = obj.get("eval_count") or 0
            eval_dur = obj.get("eval_duration") or 0  # ns
            prompt_eval_count = obj.get("prompt_eval_count") or 0
            prompt_eval_dur = obj.get("prompt_eval_duration") or 0  # ns
            load_dur = obj.get("load_duration") or 0  # ns

            if prompt_eval_count and prompt_eval_dur > 0:
                prompt_tps_samples.append(
                    prompt_eval_count / (prompt_eval_dur / 1e9)
                )
            if eval_count and eval_dur > 0:
                gen_tps_samples.append(eval_count / (eval_dur / 1e9))

            # ttft: time until first generated token ≈ model load + prompt eval.
            # Fall back to per-token prompt-eval time if prompt_eval_duration
            # is present but counts aren't, per spec.
            if prompt_eval_dur > 0:
                ttft_samples.append((load_dur + prompt_eval_dur) / 1e6)  # ms
            elif prompt_eval_count and prompt_eval_dur:
                ttft_samples.append(
                    (prompt_eval_dur / max(prompt_eval_count, 1)) / 1e6
                )

            succeeded += 1

        if progress:
            progress("aggregating", 97)

        background_load = (
            sum(load_samples) / len(load_samples) if load_samples else None
        )

        if succeeded == 0:
            return empty_result(
                model,
                self.name,
                tier,
                last_error or "all benchmark prompts failed",
                context_tested=context_tested,
                background_load=background_load,
            )

        def avg(xs: list[float]) -> Optional[float]:
            return sum(xs) / len(xs) if xs else None

        # 5. peak_mem_mb: prefer measured system-RAM delta; otherwise estimate
        # from model weight size + a context overhead margin.
        peak_mem_mb: Optional[float]
        if peak_mem_delta is not None:
            peak_mem_mb = round(peak_mem_delta, 1)
        elif model_size_mb is not None:
            # Rough proxy: weights resident + ~20% context/runtime overhead.
            peak_mem_mb = round(model_size_mb * 1.2, 1)
        else:
            peak_mem_mb = None

        return {
            "model": model,
            "adapter": self.name,
            "ok": True,
            "error": None,
            "prompt_eval_tps": _round_opt(avg(prompt_tps_samples)),
            "gen_tps": _round_opt(avg(gen_tps_samples)),
            "ttft_ms": _round_opt(avg(ttft_samples)),
            "peak_mem_mb": peak_mem_mb,
            "context_tested": context_tested,
            "background_load": _round_opt(background_load, 3),
            "tier": tier,
            "timestamp": utc_now_iso(),
        }


def _round_opt(x: Optional[float], ndigits: int = 2) -> Optional[float]:
    return round(x, ndigits) if x is not None else None
