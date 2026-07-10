// ── Tiny Tauri bridge ──────────────────────────────────────
// Wraps window.__TAURI__ so components can call backend commands and
// listen for events, and quietly no-op in a plain browser dev build.

declare global {
  interface Window {
    __TAURI__?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
      event?: {
        listen: (
          event: string,
          handler: (e: { payload: unknown }) => void
        ) => Promise<() => void>;
      };
    };
  }
}

/** True when running inside the Tauri shell (not a plain browser tab). */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && !!window.__TAURI__?.invoke;
}

/**
 * Call a backend command. Returns `null` when not running under Tauri so
 * callers can fall back to mock/empty state and keep the dev build alive.
 */
export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T | null> {
  const t = window.__TAURI__;
  if (!t?.invoke) return null;
  return (await t.invoke(cmd, args)) as T;
}

/**
 * Subscribe to a backend event. Resolves to an unlisten function, or `null`
 * when events aren't available (browser dev build).
 */
export async function listen<T>(
  event: string,
  handler: (payload: T) => void
): Promise<(() => void) | null> {
  const ev = window.__TAURI__?.event;
  if (!ev?.listen) return null;
  return ev.listen(event, (e) => handler(e.payload as T));
}
