# LlamaChat brand assets (staging)

Generated logo + full app-icon set for the **FitLLM → LlamaChat** rebrand.
These are **staged here on purpose** — nothing in the live app has been changed.
Move them into place when ready (see "Install" below).

The mark: the **Ollama llama**, vector-traced from the official artwork with
the outline strokes dilated ~2× for icon legibility ("bold trace"), rendered
white on a near-black squircle (`#0d0d12`, corner radius 22.7%). It is **one
path** with `fill-rule="evenodd"`; the outline's interior regions are true
holes, so the background shows through on transparent assets. No text on the
icon; text appears only in the wordmark lockup. The canonical path lives in
every SVG here (and in `Logo.tsx`) — they all share the identical `d` string.
Reads fully at ≥48px and stays recognizable to ~24px.

**Branding note:** this is Ollama's mascot artwork (traced, thickened), used
here deliberately as the identity of an Ollama client app. If the app ever
outgrows that context, swap in an original mark.

## Contents

```
svg/
  logo.svg          master mark — white llama on dark squircle tile, 1024×1024
  logo-mono.svg     transparent, fill=currentColor, eye is a true cut-out (UI use)
  favicon.svg       tile + llama, for the browser tab
  wordmark.svg      horizontal lockup: mark + "LlamaChat", on dark tile
  wordmark-mono.svg horizontal lockup, transparent (currentColor)
icon-1024.png       flattened 1024² master, no alpha (source of truth for rasters)
icons/              full platform set from `cargo tauri icon` (see below)
  32x32.png 64x64.png 128x128.png 128x128@2x.png icon.png icon.icns icon.ico
  Square*Logo.png StoreLogo.png            (Windows / MS Store)
  ios/AppIcon-*.png                         (18 files, no alpha, full-bleed)
  android/mipmap-*/ic_launcher*.png + xml   (adaptive icons)
Logo.tsx            drop-in React component (uses logo-mono path, currentColor)
_contact-sheet.png  preview only — safe to delete
```

## Install (when you're ready to wire it in)

Replace the existing icon set and UI assets:

- `icons/*` → copy over `src-tauri/icons/` (matches Tauri's expected names/paths;
  `tauri.conf.json`'s `bundle.icon` list already points at these).
- `svg/logo.svg` → `src-tauri/icons/logo.svg`
- `svg/favicon.svg` → `ui/public/favicon.svg`
- `Logo.tsx` → replace `ui/src/components/Logo.tsx`
- `svg/wordmark*.svg` → wherever the header/README lockup lives (optional).

## Regenerating the raster set

Everything under `icons/` is derived from `svg/logo.svg`:

```
cargo tauri icon llamachat-brand/svg/logo.svg -o llamachat-brand/icons --ios-color "#0d0d12"
```

Note: the iOS PNGs were re-flattened afterward to strip the alpha channel
(Apple rejects icons that carry alpha), and rendered full-bleed since iOS masks
its own corners. If you re-run the command above, re-strip the iOS alpha.

## The wordmark font

`wordmark.svg` / `wordmark-mono.svg` use **live text** with a system-sans stack
(`-apple-system, SF Pro Display, Segoe UI, Roboto, …`, weight 600). It renders
with whatever sans the platform has. If you need it fully self-contained
(identical everywhere, no font dependency), the text should be converted to
outlines/paths — ask and I'll produce that version.
