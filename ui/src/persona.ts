// ── Persona + mode: the two axes the whole v6 UI hangs off ─────────────────
//
// Persona (R14–R19)
//   'simple' — no model is ever shown. No context, no token counts, no tok/s,
//              no CPU/GPU/VRAM, no Library nav, no Code mode. A router picks
//              the model and the effort; the user only sees the outcome.
//   'dev'    — full control and full visibility, plus the Code workspace.
//
// Mode (R1, R2)
//   'chat' | 'cowork' | 'code'. Code is developer-only. There is exactly ONE
//   switcher for these — the segmented control inside the composer.
//
// Both are mirrored onto <html> as `data-persona` / `data-mode`, which is how
// v6 gates visibility (see the `.dev-only` / `.simple-only` rules in index.css)
// instead of scattering conditionals through JSX.

export type Persona = 'simple' | 'dev';
export type Mode = 'chat' | 'cowork' | 'code';

export const PERSONA_KEY = 'llamachat.persona';
const MODE_KEY = 'llamachat.mode';

/** The stored persona, or null when the startup question hasn't been answered. */
export function loadPersona(): Persona | null {
  try {
    const v = localStorage.getItem(PERSONA_KEY);
    return v === 'simple' || v === 'dev' ? v : null;
  } catch {
    return null;
  }
}

export function savePersona(p: Persona) {
  try { localStorage.setItem(PERSONA_KEY, p); } catch { /* ignore */ }
}

export function loadMode(): Mode {
  try {
    const v = localStorage.getItem(MODE_KEY);
    if (v === 'chat' || v === 'cowork' || v === 'code') return v;
  } catch { /* ignore */ }
  return 'chat';
}

export function saveMode(m: Mode) {
  try { localStorage.setItem(MODE_KEY, m); } catch { /* ignore */ }
}

/** Code is developer-only (R2 + v6's `.dev-only` on the Code segment). */
export function modesFor(persona: Persona): Mode[] {
  return persona === 'dev' ? ['chat', 'cowork', 'code'] : ['chat', 'cowork'];
}

export const MODE_LABEL: Record<Mode, string> = {
  chat: 'Chat',
  cowork: 'Cowork',
  code: 'Code',
};

/** Time-of-day greeting — R11 ("this looks awful except the good evening Vlad"). */
export function greeting(name: string, now: Date = new Date()): string {
  const h = now.getHours();
  const part = h < 5 ? 'Good evening' : h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening';
  return `${part}, ${name}.`;
}
