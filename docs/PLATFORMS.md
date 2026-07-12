# Cross-platform structure (macOS · Windows · Linux)

LlamaChat is a single Tauri v2 app that targets all three desktop OSes from one
codebase. This document maps where platform-specific code lives and how to build
on each OS. **macOS and Windows are fully implemented (agent input, native
screen-reading, app control); Linux is scaffolded with a working vision
fallback.**

## Layout

```
crates/llamachat-core/      Pure-Rust engine — already cross-platform
  src/hardware/          cpu / gpu / storage / apple / util — cfg per OS
  src/tools/             shell, process, filesystem, computer (open_app/type/…), desktop (screenshot)
src-tauri/               Tauri desktop shell
  src/desktop.rs         Agent desktop control (mouse/keys/read_screen/screenshot)
  src/commands.rs        Tauri commands incl. OS permission checks
  src/*.rs               chat, agent loop, memory, ollama, sidecar, settings, state
  icons/                 icon.icns (mac), icon.ico + Square*Logo (Windows), *.png (all)
  binaries/              staged Python sidecar per target triple
sidecar/                 Python benchmark/Ollama sidecar (PyInstaller onefile)
scripts/
  build-sidecar.sh       freeze sidecar on macOS / Linux
  build-sidecar.ps1      freeze sidecar on Windows
ui/                      React + Vite frontend (platform-agnostic)
.github/workflows/build.yml  3-OS matrix build (see "CI" below)
```

## CI

The 3-OS matrix build workflow lives at **`.github/workflows/build.yml`**. It
builds macOS/Windows/Linux bundles in parallel on every push to `master`, on
pull requests, and on manual `workflow_dispatch`, uploading the installers
(`.dmg` / `.msi` + `-setup.exe` / `.deb` + `.AppImage`) as run artifacts.

This is the recommended way to produce the **Windows installer**: GitHub's
`windows-latest` runners don't enforce Smart App Control, so the release
bundler's build scripts run without the `os error 4551` block described below.
Download the `.msi` / `-setup.exe` from the run's **Artifacts**.

> Note: modifying `.github/workflows/*` requires a token with the `workflow`
> scope. If a push is rejected for that reason, grant it once with
> `gh auth refresh -h github.com -s workflow` (or edit the file via GitHub's web
> UI).

### Where the OS-specific code is

| Concern | File | macOS | Windows | Linux |
| --- | --- | --- | --- | --- |
| Agent mouse/keyboard | `src-tauri/src/desktop.rs` | `mod mac` (enigo) | ✅ shared `mod input` (enigo) | ✅ shared `mod input` (enigo) |
| Agent screen read (`read_screen`) | `src-tauri/src/desktop.rs` | ✅ AX tree via `osascript` | ✅ `mod windows` — UI Automation (`uiautomation`) | ⏳ `mod linux` — TODO AT-SPI |
| Screenshot (vision perception) | `src-tauri/src/desktop.rs` `screenshot_to` | ✅ `screencapture` | ✅ PowerShell `CopyFromScreen` | ✅ `grim`/`scrot`/`import` |
| App launch / type / keys | `crates/llamachat-core/src/tools/computer.rs` | ✅ `open -a` / `osascript` | ✅ Start-Menu/App-Paths launch, `enigo` type/keys | ⏳ TODO (`xdg-open`, `xdotool`) |
| Permissions checklist | `src-tauri/src/commands.rs` | ✅ TCC (Accessibility, Screen Recording) | ✅ n/a → reported granted | ✅ n/a → reported granted |
| Hardware profile | `crates/llamachat-core/src/hardware/` | ✅ | ✅ | ✅ |

Legend: ✅ implemented · ⏳ scaffolded stub, needs a native implementation.

**Perception fallback:** where `read_screen` isn't implemented yet (Windows/Linux),
the stub returns the "no accessibility elements" marker, so the agent loop
(`src-tauri/src/agent.rs::ax_is_empty`) automatically switches to **screenshot +
vision-model** perception. So the agent can already see and drive apps on
Windows/Linux via vision before the native accessibility reader exists.

## Build prerequisites

All platforms need: **Rust** (stable), **Node 20+**, **Python 3.11+**, and the
**Tauri CLI** (`cargo install tauri-cli --version "^2" --locked`).

- **Linux** also needs: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev
  patchelf libxdo-dev` (the last is for enigo input). Screenshot vision needs
  one of `grim` (Wayland), `scrot`, or ImageMagick `import` (X11).
- **macOS**: Xcode command-line tools. Local builds sign with an Apple
  Development identity (see `tauri.conf.json` → `bundle.macOS.signingIdentity`);
  set `APPLE_SIGNING_IDENTITY=-` to ad-hoc sign instead.
- **Windows**: MSVC build tools (via Visual Studio Build Tools) and WebView2
  (preinstalled on Windows 11).

## Build steps

```bash
# 1. Frontend deps
npm --prefix ui install

# 2. Python sidecar → src-tauri/binaries/llamachat-sidecar-<triple>[.exe]
pip install -r scripts/requirements-sidecar.txt
scripts/build-sidecar.sh          # macOS / Linux
pwsh scripts/build-sidecar.ps1    # Windows

# 3. Build the app (produces the OS-native bundle)
cargo tauri build
```

Output bundles land in `target/release/bundle/` (repo-root `target/`, it's a
Cargo workspace): `.dmg`/`.app` on macOS, `.msi`/NSIS `-setup.exe` on Windows,
`.deb`/`.AppImage` on Linux.

> **Windows + Smart App Control:** `cargo tauri build` (release bundle) compiles
> extra HTML-rewriter build scripts (`selectors`/`html5ever`, pulled in by the
> `custom-protocol` frontend-embedding path). On a machine with **Smart App
> Control enforced**, SAC blocks those freshly-compiled, unsigned build-script
> executables (`os error 4551 — "An Application Control policy has blocked this
> file."`) and the bundle fails. The **app itself builds and runs fine**.
>
> **Standalone .exe without the bundler (works under SAC):** a plain *debug*
> build reuses the build-script binaries that already ran (and were allowed by
> SAC) during a normal `cargo build`, so it dodges the block:
> ```powershell
> npm --prefix ui run build                         # build ui/dist
> cargo build -p llamachat --features custom-protocol  # embeds the frontend
> Copy-Item src-tauri\binaries\llamachat-sidecar-*.exe target\debug\llamachat-sidecar.exe
> ```
> `target\debug\llamachat.exe` is then a self-contained, double-clickable app (no
> dev server needed) — copy it plus `llamachat-sidecar.exe` anywhere.
>
> For an **optimized release build or a signed `.msi`/`-setup.exe` installer**,
> use `cargo tauri dev` for local development and produce the installers on CI
> (GitHub's Windows runners don't enforce SAC; see `.github/workflows/build.yml`) or on a
> machine where SAC is off. Disabling SAC is a one-way change and is not required
> for development.

## Bundle config

`tauri.conf.json` uses `bundle.targets: "all"` (each host builds its own native
bundles). macOS-specific options (dmg layout, entitlements, signing) live under
`bundle.macOS` and are ignored elsewhere. To customize Windows/Linux packaging,
add `bundle.windows` (wix/nsis) and `bundle.linux` (deb/appimage/rpm) sections.

## Windows implementation (done)

Windows is a first-class target alongside macOS:

1. **Screen read** — `src-tauri/src/desktop.rs :: windows::read_screen` reads the
   target window's UI Automation tree (foreground window, or a named app found by
   title and brought forward) into `AXRole: label @ x,y` lines via the
   `uiautomation` crate, mirroring `mod mac`. The COM/UIA work runs on a
   dedicated MTA thread. When the tree exposes nothing useful it emits the
   "no elements" marker and the agent auto-switches to screenshot-vision.
2. **App control** — `crates/llamachat-core/src/tools/computer.rs` implements
   `open_app` (resolves Start-Menu shortcuts fuzzily, then App-Paths tokens, via
   `Start-Process`), `quit_app` (`taskkill`), `open_url`/`search_web`, and
   `type`/`key`/`click` via `enigo` (native SendInput — no permission prompt).
3. **Ollama discovery** — `src-tauri/src/ollama.rs` locates `ollama.exe` under
   `%LOCALAPPDATA%\Programs\Ollama` / `%ProgramFiles%\Ollama` (and PATH), so a
   Start-Menu-launched app finds Ollama even with a minimal inherited PATH.

Build: `pwsh scripts/build-sidecar.ps1` then `cargo tauri dev` (run) or
`cargo tauri build` (bundle — see the Smart App Control note above).

## Starting Linux work

The remaining scaffold is Linux perception: implement `mod linux::read_screen`
(AT-SPI accessibility tree) and the Linux branches of `computer.rs`
(`xdg-open` / `xdotool`). Until then the agent falls back to screenshot-vision
automatically (needs `grim`/`scrot`/ImageMagick installed), so it is usable on
Linux today.
