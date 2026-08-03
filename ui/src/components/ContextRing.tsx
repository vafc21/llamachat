import { contextFraction } from '../runtime'

const R = 9;
const CIRC = 2 * Math.PI * R; // 56.5 in v6's hardcoded stroke-dasharray

/**
 * The context-window meter (R5) — v6's `.ring`. Developer persona only; simple
 * persona manages context silently and is never asked to care about it.
 *
 * `used` is an estimate (≈4 chars/token — the UI has no tokenizer), so the
 * tooltip says so rather than implying an exact count.
 */
export function ContextRing({ used, total }: { used: number; total: number }) {
  const frac = contextFraction(used, total);
  const pct = Math.round(frac * 100);
  return (
    <span
      className="ctxwrap"
      title={`Context: ~${used.toLocaleString()} of ${total.toLocaleString()} tokens (estimated)`}
    >
      <svg className="ring" viewBox="0 0 24 24" aria-hidden="true">
        <circle className="bgc" cx="12" cy="12" r={R} />
        <circle
          className="fgc"
          cx="12" cy="12" r={R}
          strokeDasharray={CIRC.toFixed(1)}
          strokeDashoffset={(CIRC * (1 - frac)).toFixed(1)}
        />
      </svg>
      <span className="n">{pct}%</span>
    </span>
  );
}

/** The smaller ring used in the developer status bar. */
export function ContextRingMini({ used, total }: { used: number; total: number }) {
  const frac = contextFraction(used, total);
  return (
    <svg className="ring2" viewBox="0 0 24 24" aria-hidden="true">
      <circle className="bgc" cx="12" cy="12" r={R} />
      <circle
        className="fgc"
        cx="12" cy="12" r={R}
        strokeDasharray={CIRC.toFixed(1)}
        strokeDashoffset={(CIRC * (1 - frac)).toFixed(1)}
      />
    </svg>
  );
}
