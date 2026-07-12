# LlamaChat 🦙

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/core-Rust-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/shell-Tauri-24C8DB.svg)](https://tauri.app/)
[![UI: React + Tailwind](https://img.shields.io/badge/ui-React_%2B_Tailwind-61DAFB.svg)](https://react.dev/)
[![Sidecar: Python 3.11+](https://img.shields.io/badge/sidecar-Python_3.11%2B-3776AB.svg)](https://www.python.org/)

> **Which AI models will actually run on *your* machine — rated from "won't run" to "blazing", from real on-device measurements, not spec-sheet guesses.**

A Claude-Code-style terminal chat app that profiles your hardware, ranks every open model for *your* box, and lets you pull, run, and talk to them — with tools, slash commands, and permission modes. Also comes as a desktop app. No account, no telemetry, nothing leaves your machine.

---

## Install

Pick your platform. Prebuilt installers are published on the **[Releases page](https://github.com/vafc21/llamachat/releases)** and built automatically by CI whenever a version is tagged.

### Linux

#### Debian / Ubuntu / Pop!\_OS (.deb)

```bash
curl -LO https://github.com/vafc21/llamachat/releases/latest/download/LlamaChat_amd64.deb
sudo apt install ./LlamaChat_amd64.deb
```

After install, run `llamachat` from any terminal. The `.deb` registers the binary on `$PATH` and adds a desktop entry so LlamaChat appears in your app launcher.

#### Fedora / RHEL / CentOS (.rpm)

```bash
curl -LO https://github.com/vafc21/llamachat/releases/latest/download/LlamaChat.x86_64.rpm
sudo rpm -i LlamaChat.x86_64.rpm
```

#### Any Linux (.AppImage)

```bash
curl -LO https://github.com/vafc21/llamachat/releases/latest/download/LlamaChat_amd64.AppImage
chmod +x LlamaChat_amd64.AppImage
./LlamaChat_amd64.AppImage
```

The AppImage bundles everything — no package manager needed. Optionally move it to `~/.local/bin/` for the CLI, or run it as-is for the GUI.

#### Arch Linux (AUR)

Coming soon — [track this issue]().

#### cargo (Rust toolchain)

If you have Rust installed, you can also build and install the CLI-only binary (no desktop GUI) straight from crates.io:

```bash
cargo install llamachat
llamachat
```

This only builds the terminal UI and core engine — no webkit2gtk or desktop deps required.

### macOS

Download the `.dmg` from the [Releases page](https://github.com/vafc21/llamachat/releases/latest), open it, and drag LlamaChat to your Applications folder.

The app is currently **unsigned**, so the first time you launch it:

1. Right-click the app in Finder
2. Choose **Open**
3. Click **Open** in the dialog

This is normal for open-source software. See **[docs/SIGNING.md](./docs/SIGNING.md)** for the path to signed, warning-free releases.

### Windows

Download either the `.exe` installer or the `.msi` from the [Releases page](https://github.com/vafc21/llamachat/releases/latest).

- **`.exe`** — standard Windows installer, double-click to run.
- **`.msi`** — enterprise deployment, Group Policy compatible.

Windows may show *"Windows protected your PC"* on first launch. Click **More info → Run anyway**. Again, normal for unsigned open-source builds.

---

## What you get

**Terminal UI** — the default experience. Run `llamachat` in any terminal:

- **Hardware profiling** on *your* machine (CPU, GPU, VRAM, RAM, instruction sets)
- **Catalog of open models** rated Won't run → Blazing for *your* box
- **One-Enter download** — press Enter on a model, it pulls via Ollama with live progress
- **Full-screen chat** with streaming token-by-token replies
- **Slash commands** — type `/` for the palette: `/help`, `/tools`, `/permissions`, `/effort`, `/clear`, `/retry`, `/status`, `/mode`, `/quit`
- **Tools** — the model can run shell commands, read/write files, and inspect processes
- **Permission modes** (Shift+Tab to cycle):
  - `⏸ manual` — asks before every action (default)
  - `✎ accept-edits` — auto-approves file edits and safe commands
  - `◎ plan` — read-only, all writes denied
  - `▶ auto` — everything auto-approved
  - `⚠ bypass` — no prompts at all
- **Effort levels** — `/effort low | medium | high | max` controls how hard the model reasons

**Desktop app** — same engine, in a native window (Tauri + React GUI).

---

## Quick start (30 seconds)

```bash
# 1. Install (Linux .deb)
curl -LO https://github.com/vafc21/llamachat/releases/latest/download/LlamaChat_amd64.deb
sudo apt install ./LlamaChat_amd64.deb

# 2. Make sure Ollama is running (needed to run models)
ollama serve &

# 3. Launch
llamachat
```

Arrow-key through the onboarding, pick a model on the Models tab, hit Enter to download, then `r` to chat.

The desktop app (`LlamaChat` in your launcher) gives you the same hardware ratings in a graphical window — same core engine underneath.

---

## From source

Want to build it yourself? The core + CLI are pure Rust (no GUI deps), so this works anywhere:

```bash
git clone https://github.com/vafc21/llamachat.git
cd llamachat
cargo build --release
./target/release/llamachat
```

For the full desktop app, install the Tauri system deps first, then build the workspace:

```bash
# Ubuntu / Debian
sudo apt install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  libsoup-3.0-dev libjavascriptcoregtk-4.1-dev

# Everything (CLI + desktop)
cargo build --release -p llamachat
```

The UI and Python sidecar have their own dev loops — see [Dev setup](#dev-setup) below.

---

## Why LlamaChat?

Plenty of tools will tell you what *might* fit your GPU. LlamaChat is built around one thing none of them fully do: **an owned, clean-room, on-device measured benchmark loop.**

| | **LlamaChat** | whichllm | LLMFit | Run This LLM |
|---|---|---|---|---|
| On-device **measured** speed | ✅ owned harness | ❌ external quality leaderboards | ⚠️ delegates to Ollama/vLLM/MLX | ❌ spec estimates |
| Clean-room "Full Test" mode | ✅ | ❌ | ❌ | ❌ |
| Live cloud comparison | ✅ | ❌ | ❌ | ❌ |
| Local-first terminal + desktop | ✅ both | ❌ (CLI) | ❌ (TUI/web) | ❌ (web) |
| "Won't run → Blazing" tiers | ✅ | fit ranking | fit/speed scores | estimates |
| License | Apache-2.0 | MIT | MIT | not OSS |

**The differentiator:** an end-to-end benchmark loop we own (rather than delegating to a third-party runtime or pulling numbers from a community DB), a native local-first app with explicit **Won't run → Blazing** tiers, and a live, honest cloud comparison. That combination is genuine whitespace — see [`RECON.md`](./RECON.md) for the full competitive teardown.

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

## Platforms

LlamaChat builds from one codebase for **macOS, Windows, and Linux** (Tauri v2).
macOS and Windows are fully implemented; Linux is scaffolded and building (with a
working vision fallback). See **[docs/PLATFORMS.md](./docs/PLATFORMS.md)** for the full per-platform status matrix and build steps.

## Dev setup

### Prerequisites

- **Rust** — install via [rustup](https://rustup.rs/)
- **Node 24+** — for the UI
- **Python 3.11+** — for the benchmark sidecar
- **Ollama** *(optional)* — only needed to run models ([ollama.com](https://ollama.com/))

### 1. Rust core + CLI

The core and CLI are pure Rust and build with no system GUI dependencies:

```bash
cargo build
```

Try the CLI:

```bash
cargo run -p llamachat-cli               # interactive terminal UI
cargo run -p llamachat-cli -- profile    # detect hardware, print JSON
cargo run -p llamachat-cli -- catalog    # print the bundled model catalog
cargo run -p llamachat-cli -- recommend  # ranked recommendations (best-first), JSON
cargo run -p llamachat-cli -- store-info # round-trip a profile through the store
```

The installed binary is named `llamachat`.

#### Terminal UI

Running `llamachat` in a terminal launches a full-screen, Claude-Code-style
interface built on [ratatui](https://ratatui.rs): an animated llama mascot, an
arrow-key onboarding wizard, then a tabbed view of your machine and every catalog
model rated **Won't run → Blazing** for *this* box — all driven by the same core
engine, no mock data.

On the Models tab, **Enter** downloads a model (live `ollama pull` progress,
auto-starting the Ollama daemon) and **`r`** opens a full-screen **chat**:
responses stream token-by-token straight from Ollama's `/api/chat`, with a `/`
slash-command palette (↑/↓ to pick, Tab to complete), markdown rendering, and the
mascot spinner while it thinks. `Esc` interrupts a reply or returns to the model
list.

**Tools & permissions.** The model can run **shell commands** and **read/write
files** through the core tool engine. Claude-Code-style **permission modes** control
what runs without asking: `⏸ manual` (ask every time, default), `✎ accept-edits`
(auto-approve safe commands + file work), `◎ plan` (read-only), `▶ auto`
(everything auto-approved), `⚠ bypass` (no prompts). Cycle modes with
**Shift+Tab**. Type `/permissions` to manage rules, `/effort` to set reasoning
depth.

**Slash commands** (type `/` for the filterable palette): `/help` · `/commands` ·
`/tools` · `/permissions` · `/effort` · `/mode` · `/clear` · `/retry` ·
`/model` · `/status` · `/quit`. Use a tool-capable model (e.g. Llama 3.2,
Qwen 2.5) for tool use.

Verify the layout without a live terminal (handy on headless hosts / CI):

```bash
llamachat tui --selftest --screen main --size 100x30
# screens: splash | theme | profiling | ollama | models | hardware | about | chatwelcome | chatmsg
```

### 2. UI

```bash
cd ui
npm install
npm run dev
```

The UI ships a typed **mock data layer** (`ui/src/lib/api.ts`) so it renders with
sample data when run standalone (outside Tauri, i.e. `window.__TAURI__` absent).

### 3. Python sidecar

```bash
cd sidecar
pip install -e .
python -m llamachat_sidecar list-adapters
```

### 4. Tauri desktop shell

The full desktop app needs the platform webview toolkit (see [From source](#from-source) above):

```bash
cargo build -p llamachat
```

> On hosts without webkit2gtk, plain `cargo build` still works — it builds only the pure-Rust core + CLI, because they are the workspace's `default-members`.

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
- ✅ **Terminal UI** — full Claude-Code-style TUI with chat, slash palette, tools, permission modes
- ✅ **Cross-platform installers** — `.deb`, `.rpm`, `.AppImage`, `.dmg`, `.exe`, `.msi` built by CI

---

## License

Licensed under the [Apache License, Version 2.0](./LICENSE).
