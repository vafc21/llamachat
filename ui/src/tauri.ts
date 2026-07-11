// ── Tauri bridge (v2) ──────────────────────────────────────
// Uses the @tauri-apps/api package so IPC actually works in the packaged app.
// (Tauri v2 does NOT inject a global `window.__TAURI__` unless withGlobalTauri
// is set, and even then the shape differs — the package is the reliable path.)
// In a plain browser dev build (`npm run dev`, no Tauri) invoke/listen no-op to
// null so callers fall back to mock/empty state and keep the dev build alive.

import { invoke as tauriInvoke, isTauri as tauriIsTauri } from '@tauri-apps/api/core'
import { listen as tauriListen } from '@tauri-apps/api/event'

/** True when running inside the Tauri shell (not a plain browser tab). */
export function isTauri(): boolean {
  try {
    return tauriIsTauri();
  } catch {
    return false;
  }
}

/**
 * Call a backend command. Returns `null` when not running under Tauri, or when
 * the command errors (logged) — callers fall back to mock/empty state.
 */
export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T | null> {
  if (!isTauri()) return null;
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (e) {
    console.error(`invoke("${cmd}") failed:`, e);
    return null;
  }
}

/**
 * Subscribe to a backend event. Resolves to an unlisten function, or `null`
 * when events aren't available (browser dev build).
 */
export async function listen<T>(
  event: string,
  handler: (payload: T) => void
): Promise<(() => void) | null> {
  if (!isTauri()) return null;
  try {
    return await tauriListen<T>(event, (e) => handler(e.payload));
  } catch (e) {
    console.error(`listen("${event}") failed:`, e);
    return null;
  }
}
