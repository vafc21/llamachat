// ── Runtime readouts (R5, R6, R19) ─────────────────────────────────────────
//
// Everything here is derived from data the app actually has:
//   • token counts + elapsed time are measured in the UI as the stream arrives
//     (one `chat_token` event == one token, so tok/s is genuinely measured);
//   • context totals come from the selected model's `context_comfortable`;
//   • memory / VRAM come from the detected `HardwareProfile`.
//
// What is NOT available: live CPU% and GPU-utilisation%. There is no backend
// command for sampled utilisation (see `src-tauri/src/commands.rs` — the only
// hardware command is `get_hardware_profile`, which snapshots once and caches).
// Rather than animate a fake number we return `null` and the UI renders `n/a`.
// TODO(backend): add a `get_runtime_stats` command sampling CPU/GPU load and
// live VRAM so the Code readouts and the status bar can go live.

import type { HardwareProfile, Message, Recommendation } from './types'

/** Measured per-reply statistics, attached to an assistant turn. */
export interface TurnStats {
  /** Ollama tag that produced the reply. */
  model: string;
  quant?: string;
  /** Tokens streamed back (counted, not estimated). */
  tokensOut: number;
  /** Wall-clock seconds from request to last token. */
  seconds: number;
  /** Estimated prompt+reply tokens in the window at the end of the turn. */
  ctxUsed: number;
  /** The model's comfortable context size. */
  ctxTotal: number;
  /** Router line, dev persona only. Absent for manual picks. */
  router?: string;
}

/** `tokensOut / seconds`, guarded against divide-by-zero. */
export function tokensPerSec(s: TurnStats): number {
  return s.seconds > 0 ? s.tokensOut / s.seconds : 0;
}

/**
 * Rough token count for a string. ~4 chars/token is the usual English
 * approximation; we have no tokenizer in the UI, so every context figure
 * derived from this is an estimate and labelled as such.
 */
export function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

/** Estimated context occupied by a conversation. */
export function estimateContext(messages: Message[]): number {
  let n = 0;
  for (const m of messages) n += estimateTokens(m.content) + 4;
  return n;
}

/** 0–1 fraction of the context window in use. */
export function contextFraction(used: number, total: number): number {
  if (!total) return 0;
  return Math.max(0, Math.min(1, used / total));
}

// ── Hardware readouts ──────────────────────────────────────────────────────

export interface HardwareReadout {
  label: string;
  /** 0–100, or null when we have no source for it. */
  pct: number | null;
  /** Human detail, e.g. "21.4 / 64 GB". */
  detail: string;
}

const gb = (mb: number) => mb / 1024;

/**
 * CPU / RAM / GPU / VRAM readouts for the Code workspace and status bar (R6).
 *
 * RAM and VRAM are real numbers from the hardware profile (a snapshot taken at
 * detection, not a live sample). CPU and GPU utilisation have no source yet and
 * come back `pct: null` → rendered `n/a`, never invented.
 */
export function hardwareReadouts(hw: HardwareProfile | null): HardwareReadout[] {
  if (!hw) {
    return [
      { label: 'CPU', pct: null, detail: 'n/a' },
      { label: 'RAM', pct: null, detail: 'n/a' },
      { label: 'GPU', pct: null, detail: 'n/a' },
      { label: 'VRAM', pct: null, detail: 'n/a' },
    ];
  }

  const usedMb = Math.max(0, hw.memory.total_mb - hw.memory.available_mb);
  const ramPct = hw.memory.total_mb ? (usedMb / hw.memory.total_mb) * 100 : null;

  const gpu = hw.gpus[0];
  const vramTotal = gpu?.vram_total_mb ?? null;
  const vramFree = gpu?.vram_free_mb ?? null;
  const vramUsed = vramTotal !== null && vramFree !== null ? vramTotal - vramFree : null;
  const vramPct = vramTotal && vramUsed !== null ? (vramUsed / vramTotal) * 100 : null;

  return [
    {
      label: 'CPU',
      pct: null, // no sampled utilisation from the backend yet
      detail: `${hw.cpu.physical_cores}C / ${hw.cpu.logical_cores}T`,
    },
    {
      label: 'RAM',
      pct: ramPct,
      detail: `${gb(usedMb).toFixed(1)} / ${gb(hw.memory.total_mb).toFixed(0)} GB`,
    },
    {
      label: 'GPU',
      pct: null, // ditto — model/backend are known, load is not
      detail: gpu ? `${gpu.model}` : 'CPU only',
    },
    {
      label: 'VRAM',
      pct: vramPct,
      detail:
        vramTotal !== null
          ? `${vramUsed !== null ? gb(vramUsed).toFixed(1) : '?'} / ${gb(vramTotal).toFixed(0)} GB`
          : 'shared',
    },
  ];
}

/** Short one-line runtime banner: "qwen3:30b · Q4_K_M · 48 layers on GPU". */
export function runtimeLine(rec: Recommendation | null, hw: HardwareProfile | null): string {
  const parts: string[] = [];
  if (rec) {
    parts.push(rec.quant);
    const frac = rec.memory_fit.gpu_layers_fraction;
    if (rec.memory_fit.fits_gpu) parts.push('fully on GPU');
    else if (frac > 0) parts.push(`${Math.round(frac * 100)}% of layers on GPU`);
    else parts.push('CPU only');
    parts.push(`ctx ${rec.context_comfortable.toLocaleString()}`);
  }
  if (hw?.backends.length) parts.push(hw.backends.map((b) => b.toUpperCase()).join(' · '));
  return parts.join(' · ');
}

/** One entry in the Code workspace activity log — real agent tool calls. */
export interface ActivityEntry {
  id: string;
  /** Tool name, e.g. `shell`. */
  tool: string;
  /** One-line argument summary. */
  detail: string;
  state: 'running' | 'ok' | 'error';
  startedAt: number;
  endedAt?: number;
}
