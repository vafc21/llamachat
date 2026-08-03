// ── Which OS we're on ──────────────────────────────────────────────────────
//
// This matters for more than fonts. The Rust side deliberately returns `true`
// for `accessibility` and `screen_recording` on Windows and Linux, because
// neither is permission-gated there (see `commands.rs`, the
// `#[cfg(not(target_os = "macos"))]` arms). A UI that renders those booleans
// blindly would show two green "permissions granted" rows on Linux for things
// that are not permissions at all — a lie by omission.
//
// So platform is a first-class input to the readiness checklist, not just a
// font switch. Detected once and mirrored onto <html data-platform> for CSS.

export type Platform = 'linux' | 'macos' | 'windows';

/** Best-effort OS detection from the webview. */
export function detectPlatform(): Platform {
  const ua = navigator.platform || navigator.userAgent || '';
  if (ua.includes('Mac')) return 'macos';
  if (ua.includes('Win')) return 'windows';
  return 'linux';
}

/** True when this OS gates agent input / screen capture behind a TCC prompt. */
export function hasSystemPermissions(p: Platform): boolean {
  return p === 'macos';
}
