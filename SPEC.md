# Build Spec: Local LLM Optimizer (working name: **FitLLM** / **RunLocal**)

## 0. One-line pitch

An open-source, local-first desktop app that profiles your machine, runs real on-device benchmarks in the background, and tells you exactly which AI models will run on your system, rated from "won't run" to "blazing", with an honest comparison against current frontier cloud models.

Think of it as a cross between **OpenClaw** (deep, agentic system access) and **whichllm** (hardware-to-model matching), except the recommendations come from *your actual measured performance*, not theoretical specs.

---

## 1. Core principles (do not compromise on these)

1. **Local-first, zero telemetry by default.** Everything runs on the user's machine. No account, no cloud call, no data leaves the device unless the user explicitly opts in (e.g. to pull a model catalog update or submit an anonymous benchmark to a community leaderboard).
2. **Measured, not guessed.** Ratings come from benchmarks actually executed on the hardware, not a lookup table of GPU specs. Spec-based estimates are only a fallback before the first real run completes.
3. **Non-intrusive by default.** First-run profiling and background benchmarks must never freeze the machine or hog resources. The user should barely notice it running.
4. **Honest about tradeoffs.** Every recommendation shows the real cost: speed, memory headroom, quality vs. a frontier model, quantization loss.

---

## 2. Primary user flow

### 2.1 First launch (onboarding)
1. Splash + one-screen explainer of what the app does and what system access it needs, with a clear consent step.
2. **Hardware detection** runs immediately (fast, read-only, seconds).
3. Show an instant *provisional* rating using spec heuristics so the user sees value right away ("Based on your RTX 4070, you can probably run models up to ~30B quantized").
4. Kick off a **background quick benchmark** to replace those estimates with real numbers. Show a subtle progress indicator, not a blocking modal.
5. Once the quick benchmark lands, refresh the dashboard with measured ratings.

### 2.2 Ongoing use
- Dashboard shows a ranked list of models with per-model ratings and a "compare to cloud" panel.
- App can optionally keep learning: if the user actually runs a model through their runtime (Ollama, etc.), passively record real tokens/sec and feed it back into the ratings.
- Offer an on-demand **Full Test** whenever the user wants sharper numbers.

### 2.3 Full Test mode (the "clean room" benchmark)
- User-triggered, opt-in, clearly explained.
- Prompts the user to close other apps for maximum accuracy, and optionally detects/warns about heavy background processes (browsers, Docker, other GPU users).
- Runs a longer, more thorough benchmark suite with the machine as idle as possible to get best-case numbers.
- Stores both "typical" (background) and "best-case" (full test) results so the user can see the spread.

---

## 3. Architecture overview

```
+-------------------------------------------------------------+
| Desktop UI |
| (onboarding wizard, dashboard, compare view, settings) |
+----------------------------+--------------------------------+
 | IPC
+----------------------------v--------------------------------+
| Core Engine (daemon) |
| |
| +------------------+ +-----------------+ +-------------+ |
| | Hardware Profiler| | Benchmark Engine| | Recommender | |
| +------------------+ +-----------------+ +-------------+ |
| | | | |
| +-------v--------------------v--------------------v------+ |
| | Local Store (SQLite) | |
| | hardware profile, benchmark history, model catalog | |
| +-------------------------------------------------------+ |
| | |
| +--------------------------v----------------------------+ |
| | Runtime Adapters (pluggable) | |
| | Ollama | llama.cpp | vLLM | LM Studio | MLX | |
| +-------------------------------------------------------+ |
+-------------------------------------------------------------+
```

---

## 4. Module specs

### 4.1 Hardware Profiler
Detect and store, read-only, cross-platform (macOS / Linux / Windows):

- **CPU**: model, core/thread count, base/boost clocks, instruction set flags (AVX2, AVX-512, NEON).
- **GPU(s)**: vendor + model, VRAM total/free, driver/CUDA/ROCm/Metal version, compute capability. Handle multi-GPU and eGPU.
- **Apple Silicon**: detect unified memory, GPU core count, Neural Engine presence, and flag that memory is shared (this changes model-size math a lot).
- **RAM**: total + currently available.
- **Storage**: free space on the drive where models would live, plus read speed if cheaply measurable (matters for load time of big weights).
- **OS + version**, and available acceleration backends (CUDA, Metal, Vulkan, ROCm, CPU-only).

Output a normalized `HardwareProfile` object the rest of the app reads from.

### 4.2 Benchmark Engine
Two tiers:

**Quick benchmark (background, default):**
- Pull one small representative model per size class already available locally, or download a tiny probe model (a few hundred MB) if none exist, with consent.
- Measure the metrics below on a short fixed prompt set, capped in time and resource use.

**Full Test (clean room, on demand):**
- Larger prompt set, multiple size classes, multiple quant levels, longer context probes.
- Runs closer to full hardware utilization.

**Metrics to capture (both tiers):**
- **Prompt eval speed** (tokens/sec ingesting the prompt).
- **Generation speed** (tokens/sec output).
- **Time to first token (TTFT)**.
- **Peak memory used** vs. available (headroom before OOM).
- **Max practical context length** before it slows to a crawl or OOMs.
- **Thermal / throttle signal**: did sustained load drop clocks or tokens/sec over the run? Flag laptops that throttle hard.
- **Quant sensitivity**: same model at Q4 vs Q8 vs FP16 where memory allows.

Store every run with a timestamp and the machine state (background load level) so results are comparable.

### 4.3 Model Catalog
- A bundled, updatable catalog (JSON/SQLite) of open models: family, parameter count, available quant formats, memory footprint per quant, license, and a known quality score (e.g. from public eval leaderboards).
- Catalog updates are an explicit, optional network action, not automatic background phoning-home.
- Each catalog entry maps to what the runtime adapters can actually pull and run.

### 4.4 Recommendation Engine
Takes `HardwareProfile` + `BenchmarkHistory` + `ModelCatalog` and produces per-model ratings:

- **Won't run** (not enough VRAM/RAM even at smallest quant).
- **Runs, but slow** (works, under some threshold tokens/sec, or heavy swapping).
- **Runs okay** (usable interactive speed).
- **Runs great** (comfortable headroom, fast).
- **Blazing** (best-in-class for this machine).

Rating logic should blend measured tokens/sec, memory headroom, and context capacity, with configurable thresholds. Always show *why* something got its rating (e.g. "Great: 42 tok/s generation, 6GB VRAM headroom, 8k context comfortable").

### 4.5 Cloud Comparison
For each recommended local model, show an honest side-by-side against current frontier hosted models:

- **Speed**: local measured tokens/sec vs. typical hosted latency.
- **Quality**: local model's public eval scores vs. frontier model scores, clearly labeled as approximate.
- **Cost/privacy tradeoff**: local = free + private but lower ceiling; cloud = higher quality but paid + data leaves device.
- Keep the frontier model reference list in the updatable catalog so it does not go stale. Do NOT hardcode which model is "the best cloud model", since that changes constantly.

### 4.6 Runtime Adapters (pluggable)
Abstract interface so benchmarking and running work across:
- **Ollama** (easiest default target)
- **llama.cpp / GGUF** direct
- **vLLM** (for beefier Linux/CUDA rigs)
- **LM Studio** (detect if installed)
- **MLX** (Apple Silicon native)

Each adapter implements: `is_available()`, `pull(model)`, `run_benchmark(model, prompts)`, `stream_generate(...)`. Adding a new backend should mean writing one adapter, nothing else.

---

## 5. Tech stack (chosen)

- **Shell**: Tauri (Rust core + web UI) for a small, fast, cross-platform binary.
- **Core engine**: Rust for the daemon and hardware probing, with a **Python sidecar** for benchmark orchestration and ML tooling.
- **UI**: React + Tailwind.
- **Store**: SQLite (bundled, no server).
- **Packaging**: signed installers per OS; single-binary where possible.

Keep it lightweight and truly cross-platform — that is the constraint that matters.

---

## 6. Privacy + permissions
- Explicit consent screen listing exactly what is read (hardware info, optionally process list for the Full Test warning).
- No network calls except: optional catalog update, optional model download, optional community leaderboard submit. Each is separately toggleable and off-by-default where sensitive.
- All benchmark data stored locally; provide a one-click "export my data" and "wipe everything".

---

## 7. Edge cases to handle
- **CPU-only machines** (no GPU): still rate small models honestly, warn about speed.
- **Low VRAM but high system RAM**: account for CPU offload / partial GPU layers.
- **Multi-GPU and eGPU**: detect, and note that not every runtime splits across GPUs cleanly.
- **Apple unified memory**: do not apply discrete-GPU VRAM math; size limits work differently.
- **Thermal throttling laptops**: surface the sustained-vs-burst gap so users are not misled by a 10-second sprint number.
- **No runtime installed yet**: guide the user to install Ollama (or bundle a minimal llama.cpp) so first benchmark can run.
- **Big-model download size**: never auto-download multi-GB weights without a clear consent + size warning.

---

## 8. Suggested build phases

**Phase 1 (MVP):**
- Hardware profiler (all platforms).
- Ollama adapter only.
- Quick background benchmark (tokens/sec, TTFT, memory headroom).
- Static bundled model catalog.
- Dashboard with the 5-tier ratings.

**Phase 2:**
- Full Test / clean-room mode with background-load detection.
- Cloud comparison panel with updatable frontier reference list.
- llama.cpp + MLX adapters.
- Benchmark history + "typical vs best-case" spread.

**Phase 3 (stretch):**
- Passive learning from real usage.
- Optional anonymous community leaderboard ("machines like yours run X at Y tok/s").
- vLLM + LM Studio adapters.
- Auto-tuning suggestions (which quant, how many GPU layers, context size) per model.

---

## 9. Success criteria
- A new user opens the app and, within a minute, sees a *provisional* answer, and within a few minutes sees a *measured* answer, with zero manual config.
- Ratings reflect real performance on that specific machine, and update if hardware or load changes.
- The app never hangs the system, never phones home silently, and clearly explains every recommendation.

---

## 10. Before you build
Check whether **whichllm**, **LLMFit**, or **Run This LLM** already cover enough of this that a pull request beats a fresh project. The differentiator here is the *on-device measured benchmark loop* + *clean-room Full Test* + *live cloud comparison*, which none of them fully solve today. If one is close, fork it and add those; if not, greenfield it.
