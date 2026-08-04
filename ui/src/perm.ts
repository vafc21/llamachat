// ── Agent permission modes ─────────────────────────────────────────────────
//
// These strings are the WIRE FORMAT: they are exactly what `set_agent_mode`
// accepts (`PermMode::from_label` in crates/llamachat-core/src/types.rs).
// Renaming one to something friendlier without changing the Rust side makes
// the command return Err, and the backend silently keeps whatever mode it had
// — which is the class of bug this file exists to prevent.

export type AgentPermMode = 'manual' | 'accept-edits' | 'plan' | 'auto' | 'bypass';

export const PERM_MODES: AgentPermMode[] = ['manual', 'accept-edits', 'plan', 'auto', 'bypass'];

/** Plain-language labels. Rust's `label()` is the wire value, not this. */
export const PERM_LABEL: Record<AgentPermMode, string> = {
  manual: 'Ask before changes',
  'accept-edits': 'Auto-approve edits',
  plan: 'Plan only',
  auto: 'Auto-approve all',
  bypass: 'No prompts',
};

/** Mirrors `PermMode::explain()` so the menu says what each mode actually does. */
export const PERM_EXPLAIN: Record<AgentPermMode, string> = {
  manual: 'Asks before shell, file writes and process control.',
  'accept-edits': 'Auto-approves file edits and mkdir/touch/mv/cp. Still asks for the rest.',
  plan: 'Read-only. The model can look but cannot write or run commands.',
  auto: 'Everything auto-approved, with safety checks.',
  bypass: 'All tools allowed, no prompts and no checks.',
};

/** Modes that let the agent change things without asking — worth a warning tint. */
export const RISKY_MODES: AgentPermMode[] = ['auto', 'bypass'];

/** Narrow an arbitrary backend string to a known mode. */
export function asPermMode(s: string | undefined | null): AgentPermMode | null {
  return PERM_MODES.includes(s as AgentPermMode) ? (s as AgentPermMode) : null;
}
