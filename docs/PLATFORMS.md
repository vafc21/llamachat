# Cross-platform structure (macOS · Windows · Linux)

LlamaChat is a single Tauri v2 app that targets all three desktop OSes from one
codebase. This document maps where platform-specific code lives and how to build
on each OS. **macOS is the reference implementation; Windows and Linux are
scaffolded and ready to fill in.**

## Layout

```
crates/fitllm-core/      Pure-Rust engine — already cross-platform
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
.github/workflows/build.yml   3-OS matrix build
```

### Where the OS-specific code is

| Concern | File | macOS | Windows | Linux |
| --- | --- | --- | --- | --- |
| Agent mouse/keyboard | `src-tauri/src/desktop.rs` | `mod mac` (enigo) | ✅ shared `mod input` (enigo) | ✅ shared `mod input` (enigo) |
| Agent screen read (`read_screen`) | `src-tauri/src/desktop.rs` | ✅ AX tree via `osascript` | ⏳ `mod windows` — TODO UI Automation | ⏳ `mod linux` — TODO AT-SPI |
| Screenshot (vision perception) | `src-tauri/src/desktop.rs` `screenshot_to` | ✅ `screencapture` | ✅ PowerShell `CopyFromScreen` | ✅ `grim`/`scrot`/`import` |
| App launch / type / keys | `crates/fitllm-core/src/tools/computer.rs` | ✅ `open -a` / `osascript` | ⏳ TODO (`start`, SendKeys) | ⏳ TODO (`xdg-open`, `xdotool`) |
| Permissions checklist | `src-tauri/src/commands.rs` | ✅ TCC (Accessibility, Screen Recording) | ✅ n/a → reported granted | ✅ n/a → reported granted |
| Hardware profile | `crates/fitllm-core/src/hardware/` | ✅ | ✅ | ✅ |

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

# 2. Python sidecar → src-tauri/binaries/fitllm-sidecar-<triple>[.exe]
pip install -r scripts/requirements-sidecar.txt
scripts/build-sidecar.sh          # macOS / Linux
pwsh scripts/build-sidecar.ps1    # Windows

# 3. Build the app (produces the OS-native bundle)
cargo tauri build
```

Output bundles land in `target/release/bundle/` (repo-root `target/`, it's a
Cargo workspace): `.dmg`/`.app` on macOS, `.msi`/NSIS `-setup.exe` on Windows,
`.deb`/`.AppImage` on Linux.

## Bundle config

`tauri.conf.json` uses `bundle.targets: "all"` (each host builds its own native
bundles). macOS-specific options (dmg layout, entitlements, signing) live under
`bundle.macOS` and are ignored elsewhere. To customize Windows/Linux packaging,
add `bundle.windows` (wix/nsis) and `bundle.linux` (deb/appimage/rpm) sections.

## Starting Windows work

1. Implement `src-tauri/src/desktop.rs :: windows::read_screen` with the
   `uiautomation` crate (foreground window → element roles + names + screen
   rects, formatted as `AXRole: label @ x,y`). Mirror `mod mac`.
2. Implement Windows branches in `crates/fitllm-core/src/tools/computer.rs`
   (`open_app` via `start`, `type`/`key` via SendInput/SendKeys).
3. Run `pwsh scripts/build-sidecar.ps1` then `cargo tauri build`.

Until step 1 lands, the agent falls back to screenshot-vision automatically, so
it is usable on Windows today.
