// ── Skills store ───────────────────────────────────────────
// User-defined skills, invocable in chat as /<name> (like Claude Code skills).
// Stored in localStorage — they're prompt templates, no backend needed.

import type { Skill } from './types'

const KEY = 'llamachat.skills';

export function skillUid(): string {
  return 'sk-xxxxxxxx'.replace(/x/g, () => ((Math.random() * 16) | 0).toString(16));
}

/** Turn a title into a /command-safe slug. */
export function slugify(s: string): string {
  return s.toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 32);
}

// Starter skills so the feature isn't empty on first open.
const DEFAULT_SKILLS: Skill[] = [
  {
    id: 'sk-summarize', name: 'summarize', title: 'Summarize',
    description: 'Summarize the text you provide',
    instructions:
      "You are a precise summarizer. Produce a tight, faithful summary of the user's text as 3-5 bullet points. No preamble, no conclusion.",
  },
  {
    id: 'sk-explain', name: 'explain', title: 'Explain simply',
    description: 'Explain a concept in plain terms',
    instructions:
      'Explain the topic the user gives in simple, plain language a curious beginner can follow. Use one short everyday analogy. Keep it under 150 words.',
  },
  {
    id: 'sk-rewrite', name: 'rewrite', title: 'Rewrite clearly',
    description: 'Rewrite text more clearly and concisely',
    instructions:
      "Rewrite the user's text to be clearer and more concise while preserving its meaning and tone. Output only the rewrite, nothing else.",
  },
];

export function loadSkills(): Skill[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw !== null) return JSON.parse(raw) as Skill[];
    saveSkills(DEFAULT_SKILLS);
    return DEFAULT_SKILLS;
  } catch {
    return DEFAULT_SKILLS;
  }
}

export function saveSkills(skills: Skill[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(skills));
  } catch {
    /* storage may be unavailable */
  }
}
