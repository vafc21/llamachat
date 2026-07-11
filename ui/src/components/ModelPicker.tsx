import { useState } from 'react'
import type { TierModel } from '../types'

interface ModelPickerProps {
  tiers: TierModel[];
  /** Currently selected ollama tag. */
  selected: string;
  onSelect: (tag: string) => void;
  /** Open the full model catalog (all AIs). */
  onBrowseAll: () => void;
}

/** Compact status pill: Ready / NN% / Failed / Queued. */
function StatusPill({ t }: { t: TierModel }) {
  if (t.status === 'ready')
    return <span className="text-[9px] text-emerald-400">Ready</span>;
  if (t.status === 'downloading')
    return <span className="text-[9px] text-accent">{Math.round(t.pct)}%</span>;
  if (t.status === 'error')
    return <span className="text-[9px] text-red-400">Failed</span>;
  return <span className="text-[9px] text-text-muted">Queued</span>;
}

/**
 * Claude-desktop-style model switcher that lives on the input bar. Shows the
 * active tier + model; clicking opens a menu of the three tiers. Models still
 * downloading show progress and can't be selected until ready.
 */
export function ModelPicker({ tiers, selected, onSelect, onBrowseAll }: ModelPickerProps) {
  const [open, setOpen] = useState(false);

  // The active tier, or a synthetic label when a custom (non-tier) model is chosen.
  const currentTier = tiers.find((t) => t.rec.ollama_pull === selected);
  const currentIcon = currentTier?.icon ?? '●';
  const currentLabel = currentTier?.label ?? 'Custom';
  const currentName = currentTier?.rec.display_name ?? selected;

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1 text-[10px] text-text-muted hover:text-text
                   transition-colors rounded px-1 py-0.5"
        title="Switch model"
      >
        <span>{currentIcon}</span>
        <span className="text-text font-medium">{currentLabel}</span>
        <span className="text-text-muted truncate max-w-[140px]">· {currentName}</span>
        <svg width="9" height="9" viewBox="0 0 16 16" fill="none" className="opacity-70">
          <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && (
        <>
          {/* click-away layer */}
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute bottom-full mb-1.5 left-0 z-20 w-72 rounded-lg border border-border
                          bg-surface shadow-xl p-1">
            <div className="px-2 py-1 text-[9px] uppercase tracking-wide text-text-muted">Model</div>
            {tiers.map((t) => {
              const active = t.rec.ollama_pull === selected;
              const ready = t.status === 'ready';
              return (
                <button
                  key={t.tier}
                  disabled={!ready}
                  onClick={() => {
                    if (!ready) return;
                    onSelect(t.rec.ollama_pull);
                    setOpen(false);
                  }}
                  className={`w-full text-left rounded px-2 py-1.5 flex items-center gap-2 transition-colors
                    ${active ? 'bg-accent-dim' : ready ? 'hover:bg-white/[0.04]' : ''}
                    ${ready ? '' : 'opacity-60 cursor-default'}`}
                >
                  <span className="text-[13px]">{t.icon}</span>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-[11px] text-text font-medium">{t.label}</span>
                      <StatusPill t={t} />
                    </div>
                    <div className="text-[10px] text-text-muted truncate">
                      {t.rec.display_name} · smart {t.rec.intelligence_score.toFixed(0)}/10 · fast {t.rec.speed_score.toFixed(0)}/10
                    </div>
                    {t.status === 'downloading' && (
                      <div className="mt-1 h-0.5 bg-white/[0.06] rounded-full overflow-hidden">
                        <div className="h-full bg-accent rounded-full transition-all" style={{ width: `${t.pct}%` }} />
                      </div>
                    )}
                  </div>
                  {active && (
                    <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                      <path d="M3 8l3 3 7-7" stroke="var(--color-accent)" strokeWidth="2" />
                    </svg>
                  )}
                </button>
              );
            })}

            {/* Custom: the whole catalog of AIs */}
            <div className="my-1 border-t border-border" />
            <button
              onClick={() => {
                onBrowseAll();
                setOpen(false);
              }}
              className="w-full text-left rounded px-2 py-1.5 flex items-center gap-2 hover:bg-white/[0.04]
                         transition-colors text-[11px] text-text-secondary"
            >
              <span className="text-[13px]">＋</span>
              <span className="font-medium">Browse all models…</span>
            </button>
          </div>
        </>
      )}
    </div>
  );
}
