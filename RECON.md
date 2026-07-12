# LlamaChat — Competitive Recon

Date: 2026-07-09. Time-boxed recon of three existing "will this LLM run on my machine" projects, to decide whether LlamaChat should fork one or go greenfield.

**LlamaChat's differentiator:** on-device *measured* benchmark loop + clean-room "Full Test" harness + live cloud comparison, packaged as a local-first desktop app with a "Won't run → Blazing" rating.

---

## 1. whichllm (Andyyyy64/whichllm)

- **What it does:** CLI that detects your GPU/CPU/RAM and ranks HuggingFace models that fit, "ranked by real, recency-aware benchmarks, not parameter count." Supports GGUF/AWQ/GPTQ/FP16.
- **Stack:** Python 3.11+, Typer CLI, `nvidia-ml-py` + vendor backends for hardware detection, HuggingFace API.
- **On-device measured benchmarks?** **No.** The "real benchmarks" are aggregated *external quality* leaderboards (LiveBench, Artificial Analysis, Aider, Chatbot Arena ELO, Open LLM Leaderboard) combined with spec-based fit scoring. It does not run a measured inference loop on the user's machine.
- **License:** MIT.
- **Activity:** ~5.7k stars, 298 forks, latest release v0.5.15 (Jul 3, 2026). Active.
- **Interface:** CLI only, no GUI.

## 2. LLMFit (AlexsJones/llmfit)

- **What it does:** Terminal tool that right-sizes hundreds of models to your RAM/CPU/GPU, scoring across quality, speed, fit, and context. The most popular tool in the space.
- **Stack:** Rust (~80%, `ratatui` TUI, `clap`, `serde`) with some Python/JS; also a web dashboard.
- **On-device measured benchmarks?** **Partially — the closest of the three.** Two relevant features: (a) a **Community Leaderboard** of crowd-sourced measured tok/s, TTFT, VRAM from localmaxxing.com; and (b) an **"Inference Bench"** view that runs *live benchmarks against already-running providers* (Ollama, vLLM, MLX). It measures, but it **delegates to external runtimes** rather than owning a clean-room harness, and pulls/pushes to a community DB rather than being purely local-first.
- **License:** MIT.
- **Activity:** ~29.2k stars, 1.8k forks, 115 releases, latest v0.9.38 (Jul 5, 2026). Very active, large community.
- **Interface:** TUI + CLI + web dashboard. No native desktop GUI app.

## 3. Run This LLM (runthisllm.com)

- **What it does:** Web app + Chrome extension. Bidirectional lookup: hardware → compatible models, or model → required hardware. ~295+ models with build specs, VRAM requirements, and performance *estimates*. Also sells a "we'll build the machine for you" service.
- **Stack:** Closed web app; no public tech-stack detail.
- **On-device measured benchmarks?** **No.** It is a spec-lookup / estimate database. It never executes anything on the user's machine.
- **License:** **Not open source** (no repo, license, or source found). Commercial-ish, single-operator (@thomasunise).
- **Activity:** Live site; no visibility into dev cadence.
- **Interface:** Web + browser extension.

---

## Comparison

| | whichllm | LLMFit | Run This LLM |
|---|---|---|---|
| Type | CLI | TUI + CLI + web | Web app + extension |
| On-device *measured* speed | No (external quality benchmarks) | Partial (delegates to Ollama/vLLM/MLX) | No (spec estimates) |
| Clean-room own harness | No | No | No |
| Live cloud comparison | No | No | No |
| Local-first desktop GUI | No | No | No (web) |
| "Won't run → Blazing" rating | Fit ranking, not tiers | Fit/speed scores | Estimates |
| License | MIT | MIT | Not OSS |
| Stars / activity | 5.7k, active | 29.2k, very active | unknown |
| Forkable base for LlamaChat? | Weak | Partial (reference, not base) | No |

---

## Bottom line: go greenfield

**Recommendation: build LlamaChat greenfield.** None of the three is a near-perfect base to fork:

- **Run This LLM** is not open source and is spec-lookup only — irrelevant as a base.
- **whichllm** does no on-device speed measurement (its "benchmarks" are external quality leaderboards) and is a Python CLI — forking it means gutting its core and rebuilding as a desktop app.
- **LLMFit** is the only one that measures real inference, and is the strongest *reference*, but (a) it delegates measurement to third-party runtimes (Ollama/vLLM/MLX) instead of owning a clean-room harness, (b) it's a Rust TUI, not a local-first desktop GUI, and (c) it leans on a community benchmark DB rather than honest self-contained local tests. Adopting its architecture would fight LlamaChat's core thesis.

**Crucially, no competitor combines all three LlamaChat pillars** — an owned, clean-room on-device *measured* benchmark loop, a native local-first desktop app with explicit "Won't run → Blazing" tiers, and live honest cloud comparison. That whitespace is real and defensible.

**Suggested posture:** greenfield desktop app. Treat LLMFit as the reference to study (its fit-scoring heuristics, hardware-detection breadth, and community leaderboard are worth learning from — all MIT, so code/ideas are reusable), but own the measured benchmark harness end-to-end rather than inheriting anyone's architecture.
