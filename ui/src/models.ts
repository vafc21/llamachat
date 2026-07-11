// ── Tier model helpers ─────────────────────────────────────
// Turns the backend's hardware-sized LevelPlan into the three tiers the app
// auto-downloads (Quick / Smart / Best) and exposes in the chat picker. Also
// holds the browser-dev fallbacks so `npm run dev` (no Tauri backend) still runs.

import type { HardwareProfile, LevelPlan, Recommendation, TierModel } from './types'

interface TierMeta {
  id: TierModel['tier'];
  /** Which LevelPlan field this tier maps to. */
  key: 'quick' | 'standard' | 'max';
  label: string;
  icon: string;
}

/** quick→Quick (fastest), standard→Smart (balanced), max→Best (biggest that runs). */
export const TIER_META: TierMeta[] = [
  { id: 'quick', key: 'quick', label: 'Quick', icon: '⚡' },
  { id: 'smart', key: 'standard', label: 'Smart', icon: '✦' },
  { id: 'best', key: 'max', label: 'Best', icon: '★' },
];

/**
 * Build the tier list from a LevelPlan, skipping empty tiers and de-duplicating
 * when two tiers resolve to the same model (small machines collapse tiers).
 * Everything starts `pending`; the caller marks installed models `ready` and
 * updates the rest from `download_progress` events.
 */
export function tiersFromPlan(plan: LevelPlan): TierModel[] {
  const seen = new Set<string>();
  const out: TierModel[] = [];
  for (const meta of TIER_META) {
    const rec = plan[meta.key];
    if (!rec || seen.has(rec.ollama_pull)) continue;
    seen.add(rec.ollama_pull);
    out.push({ tier: meta.id, label: meta.label, icon: meta.icon, rec, status: 'pending', pct: 0 });
  }
  return out;
}

// ── Presentation helpers ───────────────────────────────────

/** Approx download size in GB (weights footprint). Mirrors the Model Library. */
export function downloadGb(rec: Recommendation): string {
  const mb = rec.memory_fit?.required_mb ?? 0;
  return mb > 0 ? (mb / 1024).toFixed(1) : '?';
}

/** One-line, plain-language description of what a model is. */
export function modelBlurb(rec: Recommendation): string {
  const n = `${rec.display_name} ${rec.ollama_pull}`.toLowerCase();
  if (n.includes('tinyllama')) return 'A tiny model for very fast, simple replies.';
  if (n.includes('phi')) return "Microsoft's compact model — quick and capable for everyday tasks.";
  if (n.includes('llama')) return "Meta's Llama — a strong, well-rounded all-purpose model.";
  if (n.includes('qwen')) return "Alibaba's Qwen — strong reasoning, coding, and multilingual.";
  if (n.includes('mistral')) return 'Mistral — an efficient, high-quality open model.';
  if (n.includes('gemma')) return "Google's Gemma — capable yet lightweight.";
  if (n.includes('deepseek')) return 'DeepSeek — tuned for strong step-by-step reasoning.';
  return rec.why || 'A local model sized to your machine.';
}

// ── Browser-dev fallbacks (no Tauri backend) ───────────────

export const MOCK_HARDWARE: HardwareProfile = {
  cpu: {
    model: 'Apple M-series (mock)',
    vendor: 'Apple',
    physical_cores: 10,
    logical_cores: 10,
    max_clock_mhz: null,
    flags: { avx2: false, avx512: false, fma: false, f16c: false, neon: true },
  },
  gpus: [
    { vendor: 'Apple', model: 'Apple GPU', vram_total_mb: null, vram_free_mb: null, backend: 'metal', cuda_version: null },
  ],
  memory: { total_mb: 24576, available_mb: 18000 },
  storage: { models_dir: '~/.ollama/models', free_mb: 500000 },
  os: { name: 'macOS', version: '26', arch: 'aarch64' },
  backends: ['metal', 'cpu'],
  detected_at: new Date().toISOString(),
};

function mockRec(over: Partial<Recommendation>): Recommendation {
  return {
    model_id: 'mock', display_name: 'Mock', params_b: 3, quality_score: 60,
    intelligence_score: 6, speed_score: 8, quant: 'Q4_K_M', tier: 'great',
    estimated_tokens_per_sec: 60, measured_tokens_per_sec: null,
    memory_fit: { required_mb: 3000, gpu_available_mb: 0, ram_available_mb: 24576, fits_gpu: false, offload: false, gpu_layers_fraction: 0 },
    context_comfortable: 8192, why: 'mock', ollama_pull: 'llama3.2:3b',
    ...over,
  };
}

/** Three mock tiers for the browser dev build. */
export function mockTiers(): TierModel[] {
  return [
    { tier: 'quick', label: 'Quick', icon: '⚡', status: 'pending', pct: 0, rec: mockRec({ display_name: 'Llama 3.2 3B', ollama_pull: 'llama3.2:3b', intelligence_score: 6, speed_score: 10 }) },
    { tier: 'smart', label: 'Smart', icon: '✦', status: 'pending', pct: 0, rec: mockRec({ display_name: 'Qwen2.5 7B', ollama_pull: 'qwen2.5:7b', intelligence_score: 7, speed_score: 8 }) },
    { tier: 'best', label: 'Best', icon: '★', status: 'pending', pct: 0, rec: mockRec({ display_name: 'Qwen3.5 9B', ollama_pull: 'qwen3.5:9b', intelligence_score: 8, speed_score: 6 }) },
  ];
}
