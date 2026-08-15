#!/usr/bin/env python3
"""
Regenerate catalog/models.json from live Ollama data.

Why a script and not a hand-edited JSON: the previous catalog claimed
`updated_at: 2026-07-09` while every entry in it was a 2024 model (Gemma 2,
Llama 3.1, Qwen 2.5, Phi-3). Hand-maintained catalogs rot silently, and the
recommender can only ever be as good as its shelf -- it was correctly picking
the best of a two-year-old list.

Sizes here are the REAL download sizes read from ollama.com/library/<f>/tags,
not estimates. If a tag 404s or its size cannot be read, the build fails rather
than shipping a catalog whose downloads break.

Usage: python3 scripts/build-catalog.py [--out catalog/models.json]
"""
import argparse
import json
import re
import sys
import urllib.request
from datetime import datetime, timezone

UA = {"User-Agent": "llamachat-catalog-builder"}

# The curated shelf. quality_score is a 0-100 scale where ~100 is a frontier
# closed model; these are normalized from vendor-published benchmark tables and
# are deliberately coarse -- treat +/-5 as noise. Ordering within a size class
# matters far more than the absolute number, because the recommender only ever
# compares models that fit the same machine.
MODELS = [
    # id, family, display, params_b, license, quality, ctx, tag, tags
    ("qwen3.5-0.8b", "Qwen", "Qwen3.5 0.8B", 0.8, "Apache-2.0", 30.0, 262144,
     "qwen3.5:0.8b", ["tiny", "instruct", "alibaba"]),
    ("granite4.1-3b", "Granite", "Granite 4.1 3B", 3.0, "Apache-2.0", 40.0, 131072,
     "granite4.1:3b", ["small", "instruct", "ibm", "tools"]),
    ("ministral3-3b", "Ministral", "Ministral 3 3B", 3.0, "Apache-2.0", 44.0, 262144,
     "ministral-3:3b", ["small", "instruct", "mistral"]),
    ("qwen3.5-4b", "Qwen", "Qwen3.5 4B", 4.0, "Apache-2.0", 52.0, 262144,
     "qwen3.5:4b", ["small", "instruct", "alibaba"]),
    ("ministral3-8b", "Ministral", "Ministral 3 8B", 8.0, "Apache-2.0", 55.0, 262144,
     "ministral-3:8b", ["mid", "instruct", "mistral"]),
    ("gemma4-e4b", "Gemma", "Gemma 4 E4B", 4.5, "Gemma", 58.0, 131072,
     "gemma4:e4b", ["mid", "instruct", "google", "vision", "audio"]),
    ("qwen3.5-9b", "Qwen", "Qwen3.5 9B", 9.0, "Apache-2.0", 62.0, 262144,
     "qwen3.5:9b", ["mid", "instruct", "alibaba"]),
    ("ministral3-14b", "Ministral", "Ministral 3 14B", 14.0, "Apache-2.0", 63.0, 262144,
     "ministral-3:14b", ["mid", "instruct", "mistral"]),
    ("gemma4-12b", "Gemma", "Gemma 4 12B", 12.0, "Gemma", 74.0, 262144,
     "gemma4:12b", ["mid", "instruct", "google", "vision", "reasoning"]),
    ("gpt-oss-20b", "gpt-oss", "gpt-oss 20B", 20.0, "Apache-2.0", 66.0, 131072,
     "gpt-oss:20b", ["large", "instruct", "openai", "reasoning"]),
    ("gemma4-26b", "Gemma", "Gemma 4 26B (MoE)", 26.0, "Gemma", 78.0, 262144,
     "gemma4:26b", ["large", "instruct", "google", "moe", "vision"]),
    ("muse-glimmer-30b", "Muse", "Muse Glimmer 30B", 29.6, "Apache-2.0", 80.0, 131072,
     "muse-glimmer:30b", ["large", "instruct", "meta", "vision"]),
    ("qwen3.6-27b", "Qwen", "Qwen3.6 27B", 27.0, "Apache-2.0", 82.0, 262144,
     "qwen3.6:27b", ["large", "instruct", "alibaba", "reasoning"]),
    ("gemma4-31b", "Gemma", "Gemma 4 31B", 30.7, "Gemma", 83.0, 262144,
     "gemma4:31b", ["large", "instruct", "google", "vision", "reasoning"]),
    ("qwen3.8-27b", "Qwen", "Qwen3.8 27B", 27.0, "Apache-2.0", 86.0, 262144,
     "qwen3.8:27b", ["large", "instruct", "alibaba", "reasoning"]),
    # Vision model used by the screen-reading agent path. Kept separate: it is
    # pulled on demand from Settings, not part of the tier plan.
    ("llava-7b", "LLaVA", "LLaVA 7B (vision)", 7.0, "Apache-2.0", 58.0, 4096,
     "llava:7b", ["vision", "multimodal", "screenshot", "agent"]),
]

# Frontier reference points, shown to contextualize local quality. Not pullable.
FRONTIER = [
    {"id": "claude-opus-5", "display_name": "Claude Opus 5", "provider": "Anthropic",
     "quality_score": 97.0, "quality_source": "Vendor benchmarks / LMArena", "typical_tps": 80.0},
    {"id": "gpt-5", "display_name": "GPT-5", "provider": "OpenAI",
     "quality_score": 96.0, "quality_source": "Vendor benchmarks / LMArena", "typical_tps": 70.0},
    {"id": "gemini-3-pro", "display_name": "Gemini 3 Pro", "provider": "Google",
     "quality_score": 95.0, "quality_source": "Vendor benchmarks / LMArena", "typical_tps": 90.0},
]


def fetch_sizes(family: str) -> dict[str, int]:
    """Tag -> download size in MB, scraped from the family's tags page."""
    url = f"https://ollama.com/library/{family}/tags"
    req = urllib.request.Request(url, headers=UA)
    html = urllib.request.urlopen(req, timeout=30).read().decode()
    out: dict[str, int] = {}
    for tag, num, unit in re.findall(
        rf"({re.escape(family)}:[a-z0-9._-]+)</.*?([0-9.]+)(GB|MB)", html, re.S
    ):
        if tag in out:
            continue
        out[tag] = round(float(num) * (1024 if unit == "GB" else 1))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="catalog/models.json")
    args = ap.parse_args()

    families = sorted({t.split(":")[0] for *_, t, _ in MODELS})
    sizes: dict[str, int] = {}
    for fam in families:
        try:
            sizes.update(fetch_sizes(fam))
        except Exception as e:  # noqa: BLE001
            print(f"FATAL: could not read tags for {fam}: {e}", file=sys.stderr)
            return 1

    models = []
    missing = []
    for mid, family, name, params, lic, quality, ctx, tag, tags in MODELS:
        size_mb = sizes.get(tag)
        if not size_mb:
            missing.append(tag)
            continue
        # Ollama's default tag is the Q4_K_M-class build; that measured number is
        # the one that matters for "will this fit". Q8/FP16 are derived from the
        # parameter count, and are only used when a machine has room to spare.
        quants = [{"name": "Q4_K_M", "bits": 4.5, "size_mb": size_mb, "ollama_tag": tag}]
        # Q8 is derived, since Ollama does not publish a size for every quant of
        # every model. Floor it above the measured 4-bit build: some default
        # tags (Gemma's QAT/multimodal e-series) are already larger than a naive
        # params-based estimate, and an 8-bit entry that claims to be *smaller*
        # than the 4-bit one would make the fit math nonsense.
        q8 = max(round(params * 1024 * 1.06), round(size_mb * 1.7))
        quants.append({"name": "Q8_0", "bits": 8.0, "size_mb": q8})
        models.append({
            "id": mid,
            "family": family,
            "display_name": name,
            "params_b": params,
            "license": lic,
            "quality_score": quality,
            "quality_source": "Vendor benchmark tables (Ollama/HF model cards), normalized",
            "context_default": min(ctx, 32768),
            "context_max": ctx,
            "quants": quants,
            "ollama_pull": tag,
            "tags": tags,
        })

    if missing:
        print(f"FATAL: these tags were not found on Ollama: {missing}", file=sys.stderr)
        print("A catalog entry whose tag does not exist is a download that fails.", file=sys.stderr)
        return 1

    catalog = {
        "schema_version": 1,
        "updated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "models": models,
        "frontier": FRONTIER,
    }
    with open(args.out, "w") as f:
        json.dump(catalog, f, indent=1)
        f.write("\n")
    print(f"wrote {args.out}: {len(models)} models, sizes verified against ollama.com")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
