import { useState } from 'react'
import type { HardwareProfile, TierModel } from '../types'
import { downloadGb } from '../models'
import { Icon } from './Icon'

interface Props {
  hardware: HardwareProfile | null;
  tiers: TierModel[];
  /** Download exactly these tiers, then continue. */
  onChoose: (tags: string[]) => void;
  onBrowseAll?: () => void;
}

function gb(mb?: number | null) {
  return mb ? (mb / 1024).toFixed(1) : '—';
}

/** Plain-language speed, from the estimated tokens/sec. */
function speedWord(tps?: number | null) {
  if (!tps) return 'Speed varies';
  if (tps >= 50) return 'Instant';
  if (tps >= 25) return 'Fast';
  if (tps >= 10) return 'Steady';
  return 'Slow but thorough';
}

const BLURB: Record<string, { head: string; body: string }> = {
  quick: {
    head: 'For quick questions',
    body: 'Answers immediately. Great for short questions, rewriting, and quick lookups.',
  },
  smart: {
    head: 'The everyday one',
    body: 'The best balance of speed and depth on this machine. Start here if you are unsure.',
  },
  best: {
    head: 'For hard problems',
    body: 'The most capable model your hardware runs. Slower, but noticeably better at reasoning, code and long documents.',
  },
};

/**
 * The model-choice screen, shown once hardware profiling is done.
 *
 * Replaces a progress bar that downloaded all three tiers unannounced -- on a
 * 12 GB card that is ~25 GB of traffic nobody agreed to. The user sees what was
 * detected, what each tier means in plain words, and picks.
 */
export function TierChoice({ hardware, tiers, onChoose, onBrowseAll }: Props) {
  // Default: the middle tier. It is the honest recommendation for most people,
  // and pre-selecting everything would recreate the download-it-all problem
  // with an extra click of consent.
  const middle = tiers.find((t) => t.tier === 'smart') ?? tiers[1] ?? tiers[0];
  const [picked, setPicked] = useState<Set<string>>(
    () => new Set(middle ? [middle.rec.ollama_pull] : []),
  );

  const toggle = (tag: string) =>
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(tag)) next.delete(tag);
      else next.add(tag);
      return next;
    });

  const allTags = tiers.map((t) => t.rec.ollama_pull);
  const allPicked = allTags.length > 0 && allTags.every((t) => picked.has(t));
  const totalGb = tiers
    .filter((t) => picked.has(t.rec.ollama_pull))
    .reduce((sum, t) => sum + (parseFloat(downloadGb(t.rec)) || 0), 0);

  const gpu = hardware?.gpus?.find((g) => (g.vram_total_mb ?? 0) > 0) ?? hardware?.gpus?.[0];

  return (
    <div className="center top">
      <h1>Pick your models.</h1>

      {/* What we found. Stated once, in one line, then out of the way. */}
      {hardware && (
        <p className="sub">
          Found {gpu?.model ?? hardware.cpu.model}
          {gpu?.vram_total_mb ? ` with ${gb(gpu.vram_total_mb)} GB of video memory` : ''}
          {' '}and {gb(hardware.memory.total_mb)} GB of memory. Here is what runs well on it.
        </p>
      )}

      <div className="tierrow">
        {tiers.map((t) => {
          const on = picked.has(t.rec.ollama_pull);
          const blurb = BLURB[t.tier] ?? { head: t.label, body: '' };
          return (
            <button
              key={t.tier}
              type="button"
              className={`tiercard${on ? ' on' : ''}`}
              aria-pressed={on}
              onClick={() => toggle(t.rec.ollama_pull)}
            >
              <div className="tch">
                <span className="tclabel">{t.label}</span>
                {on && <Icon name="check" size={15} />}
              </div>
              <b className="tchead">{blurb.head}</b>
              <p className="tcbody">{blurb.body}</p>
              <div className="tcmeta">
                <span>{t.rec.display_name}</span>
                <span>{speedWord(t.rec.estimated_tokens_per_sec)} · {downloadGb(t.rec)} GB download</span>
              </div>
            </button>
          );
        })}
      </div>

      <div className="go">
        <button
          type="button"
          className="primary"
          disabled={picked.size === 0}
          onClick={() => onChoose([...picked])}
        >
          {picked.size === 0
            ? 'Pick at least one'
            : `Download ${picked.size === 1 ? 'this model' : `these ${picked.size}`} · ${totalGb.toFixed(1)} GB`}
        </button>

        <button
          type="button"
          onClick={() => setPicked(allPicked ? new Set(middle ? [middle.rec.ollama_pull] : []) : new Set(allTags))}
        >
          {allPicked ? 'Just the recommended one' : 'Select all three'}
        </button>

        {onBrowseAll && (
          <button type="button" onClick={onBrowseAll}>Browse all models</button>
        )}

        <span className="note">
          Everything runs on this machine. You can add or remove models later in the library.
        </span>
      </div>
    </div>
  );
}
