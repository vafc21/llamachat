// ── Readiness / permission state ───────────────────────────────────────────
//
// One source of truth for "can this app actually do the thing", shared by the
// startup readiness step and the Settings panel so the two can never drift.
//
// Backing commands (all already registered in src-tauri/src/main.rs):
//   check_permissions()      → { accessibility, screen_recording, ollama }
//   request_accessibility()  → pops the macOS Accessibility prompt
//   request_screen_recording() → pops the prompt AND registers the app under
//                                Privacy ▸ Screen Recording
//   reset_permissions()      → tccutil reset, for stale ad-hoc/unsigned entries
//   restart_app()            → relaunch, required after Screen Recording
//   open_settings_pane(pane) → "accessibility" | "screen_recording" | "automation"
//
// Two macOS quirks the Rust comments call out, honoured here:
//   1. Screen Recording is cached per-process. Granting it does NOT flip
//      `screen_recording` to true until the app restarts. So we never claim
//      success off the back of `request_screen_recording()`'s return value —
//      we re-poll, and if it still reads false we say "restart required"
//      instead of pretending.
//   2. Unsigned / ad-hoc builds leave mismatched TCC entries that read as
//      "not granted" forever. `reset_permissions` is the escape hatch.

import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke, isTauri } from './tauri'

/** Exactly the shape `check_permissions` returns. */
export interface Perms {
  accessibility: boolean;
  screen_recording: boolean;
  ollama: boolean;
}

/**
 * Per-row status.
 *   'ok'       — verified granted / running by the backend
 *   'no'       — verified NOT granted / not running
 *   'checking' — a poll is in flight and we have no answer yet
 *   'unknown'  — no backend to ask (browser dev build). NOT the same as 'ok';
 *                we refuse to render a green tick we haven't verified.
 */
export type ReadyState = 'ok' | 'no' | 'checking' | 'unknown';

export interface PermissionsApi {
  perms: Perms | null;
  /** 'checking' until the first poll lands; 'unknown' with no backend. */
  state: (key: keyof Perms) => ReadyState;
  refresh: () => Promise<void>;
  /** True once at least one poll has completed. */
  polled: boolean;
  /** True while a poll is in flight. */
  busy: boolean;
}

/**
 * Live permission state.
 *
 * Re-polls: on mount, on window focus (the user grants in System Settings, in
 * another window, and comes back), and on a slow timer while anything is still
 * ungranted. Once everything reads green the timer stops — there is nothing
 * left to watch for.
 */
export function usePermissions(enabled = true): PermissionsApi {
  const [perms, setPerms] = useState<Perms | null>(null);
  const [polled, setPolled] = useState(false);
  const [busy, setBusy] = useState(false);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  const refresh = useCallback(async () => {
    if (!enabled) return;
    if (!isTauri()) { setPolled(true); return; }
    setBusy(true);
    const p = await invoke<Perms>('check_permissions');
    if (!alive.current) return;
    if (p) setPerms(p);
    setPolled(true);
    setBusy(false);
  }, [enabled]);

  useEffect(() => { refresh(); }, [refresh]);

  // Grants happen in System Settings, outside our window. Coming back to the
  // app is the strongest signal that something may have changed.
  useEffect(() => {
    if (!enabled) return;
    const onFocus = () => { refresh(); };
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onFocus);
    return () => {
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onFocus);
    };
  }, [enabled, refresh]);

  // Poll only while something is outstanding.
  const settled = perms !== null && perms.accessibility && perms.screen_recording && perms.ollama;
  useEffect(() => {
    if (!enabled || settled || !isTauri()) return;
    const t = setInterval(() => { refresh(); }, 4000);
    return () => clearInterval(t);
  }, [enabled, settled, refresh]);

  const state = useCallback(
    (key: keyof Perms): ReadyState => {
      if (!isTauri()) return 'unknown';
      if (!perms) return 'checking';
      return perms[key] ? 'ok' : 'no';
    },
    [perms]
  );

  return { perms, state, refresh, polled, busy };
}
