import { useState } from 'react'
import type { TierModel } from '../types'
import { Icon } from './Icon'

interface ModelPickerProps {
  tiers: TierModel[];
  /** Currently selected ollama tag. */
  selected: string;
  onSelect: (tag: string) => void;
  /** Open the full model catalog (all AIs). */
  onBrowseAll: () => void;
}

/** Compact status word: Ready / NN% / Failed / Queued. No emoji (R7). */
function StatusPill({ t }: { t: TierModel }) {
  if (t.status === 'ready') return <span className="tier b">Ready</span>;
  if (t.status === 'downloading') return <span className="tier">{Math.round(t.pct)}%</span>;
  if (t.status === 'error') return <span className="tier" style={{ color: 'var(--err)' }}>Failed</span>;
  return <span className="tier">Queued</span>;
}

/**
 * The manual model picker — developer persona only (R16). It shows the model
 * name plus its quant label, exactly like v6's `.modelpick`.
 *
 * The simple persona never renders this at all (R17); the composer swaps in an
 * `Auto` pill and the router decides.
 */
export function ModelPicker({ tiers, selected, onSelect, onBrowseAll }: ModelPickerProps) {
  const [open, setOpen] = useState(false);

  const current = tiers.find((t) => t.rec.ollama_pull === selected);
  const name = current?.rec.display_name ?? selected;
  const quant = current?.rec.quant ?? '';

  return (
    <div style={{ position: 'relative' }}>
      <button type="button" className="modelpick" onClick={() => setOpen((o) => !o)} title="Switch model">
        <b>{name}</b>
        {quant && <em>{quant}</em>}
        <Icon name="chev" size={14} />
      </button>

      {open && (
        <>
          <div style={{ position: 'fixed', inset: 0, zIndex: 10 }} onClick={() => setOpen(false)} />
          <div
            className="card"
            style={{
              position: 'absolute', bottom: '100%', right: 0, marginBottom: 8, zIndex: 20,
              width: 320, background: 'var(--surface2)',
            }}
          >
            <div className="cardh"><Icon name="box" /><h2>Model</h2></div>
            {tiers.map((t) => {
              const ready = t.status === 'ready';
              const active = t.rec.ollama_pull === selected;
              return (
                <div className="lrow" key={t.tier}>
                  <span className={`tier${active ? ' b' : ''}`}>{t.label}</span>
                  <div className="nm">
                    <b>{t.rec.display_name}</b>
                    <span>{t.rec.ollama_pull} · {t.rec.quant}</span>
                  </div>
                  <StatusPill t={t} />
                  <button
                    type="button"
                    className={`act${active ? ' on' : ''}`}
                    disabled={!ready}
                    onClick={() => { if (ready) { onSelect(t.rec.ollama_pull); setOpen(false); } }}
                  >
                    {active ? 'In use' : ready ? 'Use' : '—'}
                  </button>
                </div>
              );
            })}
            <div className="lrow">
              <button
                type="button"
                className="act"
                style={{ marginLeft: 'auto' }}
                onClick={() => { onBrowseAll(); setOpen(false); }}
              >
                Browse all models
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
