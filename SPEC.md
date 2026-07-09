# Build Spec: FitLLM

## 0. One-line pitch

**A cross-platform local AI assistant that runs on YOUR machine.** First it profiles your hardware, picks the best model you can actually run, downloads it, and then gives you a full agent with shell, filesystem, browser, and process control — all running locally, zero cloud.

Think of it as **OpenClaw running on local models** instead of cloud APIs. The hardware profiling + model selection is the onboarding wizard. The product is the assistant.

---

## 1. Core principles (do not compromise on these)

1. **Local-first, zero telemetry by default.** Everything runs on the user's machine. No account, no cloud call, no data leaves the device unless the user explicitly opts in (e.g. to pull a model catalog update or submit an anonymous benchmark to a community leaderboard).
2. **Measured, not guessed.** Ratings come from benchmarks actually executed on the hardware, not a lookup table of GPU specs. Spec-based estimates are only a fallback before the first real run completes.
3. **Non-intrusive by default.** First-run profiling and background benchmarks must never freeze the machine or hog resources. The user should barely notice it running.
4. **Honest about tradeoffs.** Every recommendation shows the real cost: speed, memory headroom, quality vs. a frontier model, quantization loss.

---

## 2. Primary user flow

### 2.1 First launch (onboarding wizard)
1. Welcome screen — one sentence: "FitLLM runs AI locally on your machine. First, let's find the best model for your hardware."
2. **Hardware detection** runs immediately (fast, read-only, seconds).
3. Show the best recommended model with a one-line explanation of why.
4. User clicks "Download & Start" — pulls the model via Ollama/llama.cpp with a progress bar.
5. Model loaded → you're in the assistant.

### 2.2 The assistant (main product)
- Chat-first interface: you talk to the model, it responds.
- Full tool access (user grants on first use):
  - **Shell**: run commands, get output
  - **Filesystem**: read, write, edit files
  - **Browser**: open URLs, take screenshots, interact with pages
  - **Process**: start/stop/monitor background processes
  - **System**: hardware info, resource usage, notifications
- Tool output renders inline — code blocks, file diffs, screenshots, terminal output
- Conversation history persisted locally in SQLite
- Multiple conversations / threads in a sidebar

### 2.3 Settings & model management
- Switch models anytime (shows what's installed + what else fits)
- Full benchmark re-run for sharper numbers
- Download/delete models
- Privacy controls: all data is local, export/wipe available

---

## 3. Architecture overview

```
+-------------------------------------------------------------+
| Desktop UI (React + Tailwind)                               |
| +-------------------+--------------------------------------+|
| | Sidebar           | Main area                             ||
| | - Conversations   | +----------------------------------+ ||
| | - Model status    | | Chat (messages, tool output,     | ||
| | - Settings gear   | |  code blocks, diffs, terminal)   | ||
| |                   | +----------------------------------+ ||
| |                   | +----------------------------------+ ||
| |                   | | Input bar (prompt + tool toggle)  | ||
| |                   | +----------------------------------+ ||
| +-------------------+--------------------------------------+|
+----------------------------+--------------------------------+
                             | IPC (Tauri commands + events)
+----------------------------v--------------------------------+
| Core Engine (Rust)                                          |
| +------------+ +------------+ +-------------+ +----------+  |
| | Profiler   | | Recommender| | Tool Engine | | Store    |  |
| +------------+ +------------+ +-------------+ +----------+  |
|      |              |              |               |        |
| +----v--------------v--------------v---------------v------+  |
| |                   SQLite Store                          |  |
| | hardware / models / benchmarks / conversations / tools  |  |
| +--------------------------------------------------------+  |
|      |                                                      |
| +----v---------------------------------------------------+  |
| | Runtime Adapter (Python sidecar)                        |  |
| | - Ollama (primary)                                      |  |
| | - llama.cpp (bundled fallback)                          |  |
| | - MLX (Apple Silicon)                                   |  |
| +--------------------------------------------------------+  |
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

### 4.7 Tool Engine (the assistant's hands)
The assistant can use these tools, each with user-facing consent and per-invocation approval for destructive actions:

- **Shell** (`exec`): run commands, capture stdout/stderr, timeout, working directory. Whitelist/blacklist for safety.
- **Filesystem** (`read`, `write`, `edit`): read files (with size limits), write new files, targeted edits. Respects OS file permissions.
- **Browser** (`navigate`, `screenshot`, `click`, `type`): control a headless or visible browser. Useful for web research, form filling, testing.
- **Process** (`spawn`, `list`, `kill`): start background tasks, monitor running processes.
- **System** (`profile`, `resources`): read hardware profile, check current CPU/RAM/GPU usage.

All tool calls are:
- Rendered inline in the chat (code blocks for shell, diffs for edits, images for screenshots)
- Logged to the local store for audit
- Rate-limited and resource-capped (no fork bombs, no infinite loops)

### 4.8 Assistant Engine
- Manages the conversation loop: user message → model inference → tool calls → tool results → model continues → final response
- Streaming token output to the UI
- Handles tool-use parsing (model outputs a structured tool call, engine executes it, feeds result back)
- Context window management (truncation, summarization for long conversations)
- Multiple concurrent conversations, each with isolated context

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

## 8. Build phases

**Phase 1 (MVP — DONE ✅):**
- Hardware profiler (Linux; Mac/Win stubs).
- Ollama adapter with quick benchmark.
- Static bundled model catalog + 5-tier recommendation engine.
- SQLite local store.
- CLI (`fitllm profile|catalog|recommend|store-info`).
- Dashboard UI showing hardware + recommendations.

**Phase 2 (Assistant — current):**
- Complete UI redesign: chat-first, tool-native, dark-only, dense.
- Setup wizard: profile → recommend → download → chat.
- Tool engine: shell, filesystem, browser, process, system.
- Streaming chat with tool-use loop.
- Conversation persistence + sidebar.
- Cross-platform hardware profiler (Mac/Windows real implementations).
- Model download + management in UI.

**Phase 3 (Polish):**
- Full Test / clean-room benchmark with background-load detection.
- llama.cpp + MLX adapters (for self-contained inference).
- Cloud comparison panel with updatable frontier list.
- Benchmark history + "typical vs best-case" spread.
- Passive learning from real usage.

**Phase 4 (Stretch):**
- Optional anonymous community leaderboard.
- vLLM + LM Studio adapters.
- Auto-tuning suggestions (quant, GPU layers, context size).
- Plugin system for custom tools.

---

## 9. Success criteria
- A new user opens the app and, within a minute, sees a *provisional* answer, and within a few minutes sees a *measured* answer, with zero manual config.
- Ratings reflect real performance on that specific machine, and update if hardware or load changes.
- The app never hangs the system, never phones home silently, and clearly explains every recommendation.

---

## 10. Before you build
Check whether **whichllm**, **LLMFit**, or **Run This LLM** already cover enough of this that a pull request beats a fresh project. The differentiator here is the *on-device measured benchmark loop* + *clean-room Full Test* + *live cloud comparison*, which none of them fully solve today. If one is close, fork it and add those; if not, greenfield it.
