// ── Slash-command registry ─────────────────────────────────
// Claude-Code-style `/` commands. Built-ins here; skills are appended at runtime
// (each user skill becomes /<name>). The InputBar shows a `/` autocomplete menu;
// App executes the selected command.

import type { Skill } from './types'

export type CommandKind = 'builtin' | 'tool' | 'skill';

export interface SlashCommand {
  /** Name without the leading slash. */
  name: string;
  description: string;
  /** Hint shown after the name, e.g. "<command>". */
  argHint?: string;
  /** When true, the menu fills `/name ` and waits for args instead of running. */
  takesArgs?: boolean;
  kind: CommandKind;
}

export const BUILTIN_COMMANDS: SlashCommand[] = [
  { name: 'new', description: 'Start a new conversation', kind: 'builtin' },
  { name: 'clear', description: 'Clear this conversation', kind: 'builtin' },
  { name: 'help', description: 'List available commands', kind: 'builtin' },
  { name: 'model', description: 'Switch model — quick | smart | best', argHint: '[tier]', kind: 'builtin' },
  { name: 'models', description: 'Browse all models', kind: 'builtin' },
  { name: 'skills', description: 'Create & manage skills', kind: 'builtin' },
  { name: 'memory', description: 'View & edit long-term memory', kind: 'builtin' },
  { name: 'remember', description: 'Save a fact to long-term memory', argHint: '<fact>', takesArgs: true, kind: 'builtin' },
  { name: 'forget', description: 'Remove matching facts from memory', argHint: '<text>', takesArgs: true, kind: 'builtin' },
  { name: 'settings', description: 'Open settings', kind: 'builtin' },
  { name: 'copy', description: 'Copy the last reply', kind: 'builtin' },
  { name: 'retry', description: 'Regenerate the last reply', kind: 'builtin' },
  { name: 'system', description: "Set this chat's system prompt", argHint: '<prompt>', takesArgs: true, kind: 'builtin' },
  { name: 'shell', description: 'Run a shell command', argHint: '<command>', takesArgs: true, kind: 'tool' },
  { name: 'file', description: 'read <path>  |  write <path> <text>', argHint: '<action> <path>', takesArgs: true, kind: 'tool' },
  { name: 'browser', description: 'Open a URL in your browser', argHint: '<url>', takesArgs: true, kind: 'tool' },
];

/** Full command list = built-ins + one entry per skill. */
export function allCommands(skills: Skill[]): SlashCommand[] {
  return [
    ...BUILTIN_COMMANDS,
    ...skills.map((s): SlashCommand => ({
      name: s.name,
      description: s.description || s.title,
      argHint: '<text>',
      takesArgs: true,
      kind: 'skill',
    })),
  ];
}

/** Parse "/shell ls -la" → { name: 'shell', args: 'ls -la' }, or null if not a command. */
export function parseCommand(input: string): { name: string; args: string } | null {
  if (!input.startsWith('/')) return null;
  const m = input.slice(1).match(/^(\S+)\s*([\s\S]*)$/);
  if (!m) return null;
  return { name: m[1].toLowerCase(), args: m[2].trim() };
}

/**
 * If the input is a bare `/partial` (no space yet), return the partial for
 * menu filtering; otherwise null (menu hidden once args are being typed).
 */
export function menuQuery(input: string): string | null {
  const m = input.match(/^\/(\S*)$/);
  return m ? m[1].toLowerCase() : null;
}
