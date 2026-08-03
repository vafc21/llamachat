// ── UI-only preferences ────────────────────────────────────────────────────
// The toggles in v6's Settings that have no backend field yet. Kept out of
// AppSettings deliberately: these are presentation choices, not runtime config
// the Rust side needs to know about.

const PREFS_KEY = 'llamachat.uiPrefs'

export interface UiPrefs {
  /** Dev: show the runtime status bar along the bottom. */
  statusBar: boolean;
  /** Dev: print the router's decision above each reply. */
  explainRouter: boolean;
  /** Simple: let the router pick, vs. always using the everyday model. */
  autoRoute: boolean;
  /** Simple: bias the router toward the faster tier. */
  preferSpeed: boolean;
}

export const DEFAULT_PREFS: UiPrefs = {
  statusBar: true,
  explainRouter: true,
  autoRoute: true,
  preferSpeed: false,
};

export function loadPrefs(): UiPrefs {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (raw) return { ...DEFAULT_PREFS, ...(JSON.parse(raw) as Partial<UiPrefs>) };
  } catch { /* ignore */ }
  return DEFAULT_PREFS;
}

export function savePrefs(p: UiPrefs) {
  try { localStorage.setItem(PREFS_KEY, JSON.stringify(p)); } catch { /* ignore */ }
}
