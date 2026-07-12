# LlamaChat

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/core-Rust-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/shell-Tauri-24C8DB.svg)](https://tauri.app/)
[![UI: React + Tailwind](https://img.shields.io/badge/ui-React_%2B_Tailwind-61DAFB.svg)](https://react.dev/)
[![Sidecar: Python 3.11+](https://img.shields.io/badge/sidecar-Python_3.11%2B-3776AB.svg)](https://www.python.org/)
[![Status: Phase 1](https://img.shields.io/badge/status-Phase_1_MVP-yellow.svg)](#phase-1-status)

> **Which AI models will actually run on *your* machine — rated from "won't run" to "blazing", from real on-device measurements, not spec-sheet guesses.**

LlamaChat is an open-source, **local-first** desktop app that profiles your hardware, runs real inference benchmarks in the background, and ranks open models by how well they run on *your* system. It pairs that with an honest side-by-side against current frontier cloud models, so you can see the true speed / quality / privacy tradeoff before you download a single weight. No account, no telemetry, nothing leaves your device unless you explicitly opt in.

---

## Why LlamaChat?

Plenty of tools will tell you what *might* fit your GPU. LlamaChat is built around one thing none of them fully do: **an owned, clean-room, on-device measured benchmark loop.**

| | **LlamaChat** | whichllm | LLMFit | Run This LLM |
|---|---|---|---|---|
| On-device **measured** speed | ✅ owned harness | ❌ external quality leaderboards | ⚠️ delegates to Ollama/vLLM/MLX | ❌ spec estimates |
| Clean-room "Full Test" mode | ✅ | ❌ | ❌ | ❌ |
| Live cloud comparison | ✅ | ❌ | ❌ | ❌ |
| Local-first desktop GUI | ✅ | ❌ (CLI) | ❌ (TUI/web) | ❌ (web) |
| "Won't run → Blazing" tiers | ✅ | fit ranking | fit/speed scores | estimates |
| License | Apache-2.0 | MIT | MIT | not OSS |

**The differentiator:** an end-to-end benchmark loop we own (rather than delegating to a third-party runtime or pulling numbers from a community DB), a native local-first desktop app with explicit **Won't run → Blazing** tiers, and a live, honest cloud comparison. That combination is genuine whitespace — see [`RECON.md`](./RECON.md) for the full competitive teardown.

---

## Platforms

LlamaChat builds from one codebase for **macOS, Windows, and Linux** (Tauri v2).
macOS and Windows are fully implemented; Linux is scaffolded and building (with a
working vision fallback). Per-OS code boundaries, the implemented/TODO status
matrix, and per-platform build steps live in **[docs/PLATFORMS.md](./docs/PLATFORMS.md)**.

| | macOS | Windows | Linux |
|---|---|---|---|
| Chat · models · memory · skills | ✅ | ✅ | ✅ |
| Agent input (mouse/keys) | ✅ | ✅ enigo | ✅ enigo |
| Agent app control (open/type/keys) | ✅ | ✅ Start-Menu launch + enigo | ⏳ TODO (vision fallback works) |
| Agent screen-read | ✅ AX tree | ✅ UI Automation | ⏳ TODO (vision fallback works) |
| Screenshot vision | ✅ | ✅ | ✅ |
| Sidecar build | `scripts/build-sidecar.sh` | `scripts/build-sidecar.ps1` | `scripts/build-sidecar.sh` |

### Download & install

Prebuilt installers for macOS, Windows, and Linux are published on the
[Releases page](https://github.com/vafc21/llamachat/releases) (built by CI when a
version is tagged). The installers are currently **unsigned**, so on first launch:

- **Windows** may show *"Windows protected your PC"* → click **More info → Run anyway**.
- **macOS** → right-click the app → **Open** (once).

This is normal for open-source software. See **[docs/SIGNING.md](./docs/SIGNING.md)**
for the free (open-source) path to code-signed, warning-free downloads.

---

## Architecture

```
+-------------------------------------------------------------+
| Desktop UI                                                  |
| (onboarding wizard, dashboard, compare view, settings)      |
+----------------------------+--------------------------------+
                             | IPC (Tauri commands)
+----------------------------v--------------------------------+
| Core Engine (Rust)                                          |
|                                                             |
| +------------------+ +-----------------+ +-------------+     |
| | Hardware Profiler| | Benchmark Engine| | Recommender |     |
| +------------------+ +-----------------+ +-------------+     |
|          |                   |                  |           |
| +--------v-------------------v------------------v--------+   |
| | Local Store (SQLite)                                  |   |
| | hardware profile, benchmark history, model catalog   |   |
| +------------------------------------------------------+   |
|          |                                                  |
| +--------v----------------------------------------------+   |
| | Runtime Adapters (pluggable, via Python sidecar)     |   |
| | Ollama | llama.cpp | vLLM | LM Studio | MLX          |   |
| +------------------------------------------------------+   |
+-------------------------------------------------------------+
```

The **core engine** is pure Rust with no GUI dependencies, so it builds and runs anywhere (CI, headless servers, machines without webkit2gtk). The **Tauri shell** and the **`llamachat` CLI** both depend on it. Benchmark orchestration lives in a **Python sidecar** so a new runtime backend is one Python file.

See [`SPEC.md`](./SPEC.md) for the full design and [`CONTRACT.md`](./CONTRACT.md) for the frozen inter-module interfaces.

---

## Tech stack

| Layer | Choice |
|---|---|
| Desktop shell | [Tauri](https://tauri.app/) — small, fast, cross-platform binary |
| Core engine | Rust (`llamachat-core`) — hardware probing, catalog, recommender, store |
| CLI | Rust (`llamachat-cli`) + [clap](https://docs.rs/clap) |
| Benchmark sidecar | Python 3.11+ (`llamachat_sidecar`) |
| UI | React + Tailwind + Vite |
| Local store | SQLite (bundled via `rusqlite`, no server) |

### Workspace layout

```
llamachat/
  Cargo.toml               # workspace; default-members = core + cli (build w/o webkit)
  crates/
    llamachat-core/           # pure-Rust lib: types, hardware, catalog, recommend, store
    llamachat-cli/            # `llamachat` binary — exercises the core without the GUI
  catalog/models.json      # bundled model data
  sidecar/                 # Python benchmark orchestration
  ui/                      # React + Tailwind + Vite dashboard
  src-tauri/               # Tauri shell (added once scaffolded; needs webkit2gtk)
```

---

## Dev setup

### Prerequisites

- **Rust** — install via [rustup](https://rustup.rs/)
- **Node 24+** — for the UI
- **Python 3.11+** — for the benchmark sidecar
- **Ollama** *(optional)* — only needed to actually run benchmarks ([ollama.com](https://ollama.com/))

### 1. Rust core + CLI

The core and CLI are pure Rust and build with no system GUI dependencies:

```bash
cargo build
```

Try the CLI:

```bash
cargo run -p llamachat-cli               # interactive terminal UI (in a real terminal)
cargo run -p llamachat-cli -- tui        # ...the same UI, explicitly
cargo run -p llamachat-cli -- profile    # detect hardware, print JSON
cargo run -p llamachat-cli -- catalog    # print the bundled model catalog
cargo run -p llamachat-cli -- recommend  # ranked recommendations (best-first), JSON
cargo run -p llamachat-cli -- store-info # round-trip a profile through the store
```

The installed binary is named `llamachat`.

#### Terminal UI

Running `llamachat` in a terminal (or `llamachat tui`) launches a full-screen,
Claude-Code-style interface built on [ratatui](https://ratatui.rs): an animated
llama mascot, an arrow-key onboarding wizard (theme → live hardware profiling →
Ollama check), then a tabbed view of your machine and every catalog model rated
**Won't run → Blazing** for *this* box — all driven by the same core engine, no
mock data. Piped or redirected (non-interactive), `llamachat` prints the scriptable
help summary instead, and the JSON subcommands above are unchanged.

On the Models tab, **Enter** downloads a model (live `ollama pull` progress,
auto-starting the Ollama daemon) and **`r`** opens a full-screen **chat**:
responses stream token-by-token straight from Ollama's `/api/chat`, with a `/`
slash-command palette (`/help`, `/clear`, `/retry`, `/models`, `/quit`),
markdown rendering, and the mascot spinner while it thinks. `Esc` interrupts a
reply or returns to the model list.

Verify the layout without a live terminal (handy on headless hosts / CI):

```bash
llamachat tui --selftest --screen main --size 100x30   # splash|theme|profiling|ollama|models|hardware|about
```

### 2. UI

```bash
cd ui
npm install
npm run dev
```

The UI ships a typed **mock data layer** (`ui/src/lib/api.ts`) so it renders with sample data when run standalone (outside Tauri, i.e. `window.__TAURI__` absent).

### 3. Python sidecar

```bash
cd sidecar
pip install -e .
python -m llamachat_sidecar list-adapters
```

### 4. Tauri desktop shell

The full desktop app needs the platform webview toolkit. On Linux that means webkit2gtk and friends (see the table below):

```bash
cargo build -p llamachat
```

> On hosts without webkit2gtk, plain `cargo build` still works — it builds only the pure-Rust core + CLI, because they are the workspace's `default-members`.

---

## System dependencies (Ubuntu 24.04)

The Tauri shell needs these packages to build/link. The Rust core + CLI need **none** of them.

| Package | Provides |
|---|---|
| `libwebkit2gtk-4.1-dev` | WebKitGTK webview (renders the UI) |
| `libgtk-3-dev` | GTK 3 windowing/toolkit |
| `libayatana-appindicator3-dev` | System tray / app indicator |
| `librsvg2-dev` | SVG rendering (icons) |
| `libsoup-3.0-dev` | HTTP/networking for the webview |
| `libjavascriptcoregtk-4.1-dev` | JavaScriptCore engine for the webview |

Install them all:

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev
```

---

## Phase 1 status

Phase 1 (MVP) scope and where each piece currently stands:

- ✅ **Hardware profiler** — CPU/GPU/RAM/storage/OS detection via `sysinfo` + `nvidia-smi`
- ✅ **Ollama adapter** — Python sidecar with full ollama integration and benchmark harness
- ✅ **Quick benchmark** — background tokens/sec, TTFT, memory headroom via ollama generate API
- ✅ **Model catalog** — 14 bundled open models with quants, quality scores, and ollama pull tags
- ✅ **Recommendation engine** — 5-tier "Won't run → Blazing" ratings with human-readable *why*
- ✅ **Dashboard UI** — React + Tailwind with hardware panel, tiered recs, onboarding wizard
- ✅ **CLI** — `profile` / `catalog` / `recommend` / `store-info` all wired to real implementations
- ✅ **Tauri shell** — scaffolded with IPC commands, background benchmark events, consent flow
- ✅ **Shared type contract** — `types.rs` + `CONTRACT.md` frozen across all modules
- ✅ **Tool system** — sidecar exposes a `shell` tool; local models emit structured `{"tool": ...}` calls
- ✅ **Agent loop** — sidecar drives the model→tool→result cycle for on-device agentic runs

Phase 1 is functionally complete. Try it:
```bash
cargo run -p llamachat-cli -- profile     # real hardware data
cargo run -p llamachat-cli -- recommend   # ranked model recommendations
python -m llamachat_sidecar benchmark --adapter ollama --model llama3.2:1b --tier quick
```

---

## Known gaps

- **Tauri shell won't build on this host** until webkit2gtk system deps are installed (see table above). The Rust core + CLI + UI dev server all work without them.
- **Cloud comparison** (frontier side-by-side) is Phase 2; the catalog already carries a `frontier` reference list so it can ship without a code change.
- **Adapters beyond Ollama** (llama.cpp, vLLM, LM Studio, MLX) are Phase 2/3.
- **Needs `sudo`:** installing the Ubuntu system packages for the Tauri shell (`apt install ...`) requires root. Nothing in the core or CLI requires elevated privileges.

---

## License

Licensed under the [Apache License, Version 2.0](./LICENSE).
