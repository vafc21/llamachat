import { useState, useEffect } from 'react'
import type { HardwareProfile, TierModel } from '../types'
import { downloadGb, modelBlurb } from '../models'
import { Icon } from './Icon'
import type { Persona } from '../persona'

interface Props {
  /** 'profiling' while detecting hardware, 'setup' while pulling models. */
  phase: 'profiling' | 'setup';
  hardware: HardwareProfile | null;
  tiers: TierModel[];
  /** Which setup was chosen at the startup question — changes how much we say. */
  persona: Persona;
  /** Continue to chat manually (offered if the Quick model fails to download). */
  onContinue: () => void;
  /** Open the full model catalog (all AIs) to pick or add a custom one. */
  onBrowseAll: () => void;
}

/**
 * First-run onboarding — purely presentational. App.tsx drives the real work
 * (hardware detection, auto-downloading the three tier models) and feeds live
 * state in as props. Chat opens the moment the Quick model is ready.
 *
 * Runs AFTER the persona question, and honours it: R17 says the simple persona
 * is never shown model names, so it gets a progress line and nothing else,
 * while developers get the full hardware + per-tier breakdown.
 */
export function SetupWizard({ phase, hardware, tiers, persona, onContinue, onBrowseAll }: Props) {
  const quick = tiers[0];
  const quickFailed = quick?.status === 'error';

  // A download that stalls without ever emitting an error used to leave the
  // simple persona on this screen with no button at all — no skip, no back, no
  // explanation. Offer an escape once it's clearly not progressing.
  const [stalled, setStalled] = useState(false);
  useEffect(() => {
    if (phase !== 'setup') return;
    setStalled(false);
    const t = setTimeout(() => setStalled(true), 45000);
    return () => clearTimeout(t);
  }, [phase, quick?.pct, quick?.status]);
  const canSkip = quickFailed || stalled;
  const totalGb = tiers.reduce((sum, t) => sum + (parseFloat(downloadGb(t.rec)) || 0), 0);
  const done = tiers.filter((t) => t.status === 'ready').length;
  const overall = tiers.length ? tiers.reduce((s, t) => s + (t.status === 'ready' ? 100 : t.pct), 0) / tiers.length : 0;

  return (
    <div className="setup">
      <div className="mk"><Icon name="llama" /></div>

      {phase === 'profiling' && (
        <>
          <h1>Getting to know your machine.</h1>
          <p className="sub">
            Reading CPU, GPU, RAM and storage so LlamaChat can size its models to what you actually have.
            Nothing leaves your device.
          </p>
        </>
      )}

      {phase === 'setup' && (
        <>
          <h1>Setting things up.</h1>
          <p className="sub">
            {persona === 'simple'
              ? 'Downloading the models that fit this machine. You can start as soon as the first one lands.'
              : `Downloading a Quick, Smart and Best model sized to your machine${totalGb > 0 ? ` (~${totalGb.toFixed(1)} GB total)` : ''}. Chat opens as soon as Quick is ready — the rest finish in the background.`}
          </p>

          <div style={{ width: '100%', maxWidth: 660 }}>
            {/* Simple persona: one honest progress line, no model names (R17). */}
            {persona === 'simple' && (
              <div className="card">
                <div className="cardh">
                  <Icon name="box" /><h2>Downloading</h2>
                  <span className="tag">{done} of {tiers.length || '—'} ready</span>
                </div>
                <div className="log">
                  <div className="lg">
                    <Icon name="dash" />
                    <span className="mini" style={{ width: 200 }}><i className="a" style={{ width: `${overall}%` }} /></span>
                    <span className="p">{Math.round(overall)}%</span>
                  </div>
                </div>
              </div>
            )}

            {/* Developer: the machine + every tier, in full (R16, R19). */}
            {persona === 'dev' && hardware && (
              <div className="card">
                <div className="cardh"><Icon name="cpu" /><h2>This machine</h2>
                  <span className="tag">{hardware.os.name} {hardware.os.arch}</span></div>
                <div className="cli">
                  <div><span className="d">cpu:</span> <span className="k">{hardware.cpu.model}</span> · {hardware.cpu.physical_cores}C / {hardware.cpu.logical_cores}T</div>
                  <div><span className="d">gpu:</span> <span className="k">{hardware.gpus[0]?.model ?? 'none detected'}</span>
                    {hardware.gpus[0]?.vram_total_mb ? <> · {(hardware.gpus[0].vram_total_mb / 1024).toFixed(0)} GiB VRAM</> : null}</div>
                  <div><span className="d">memory:</span> <span className="k">{(hardware.memory.total_mb / 1024).toFixed(0)} GiB</span> · {(hardware.memory.available_mb / 1024).toFixed(0)} GiB free</div>
                  <div><span className="d">backends:</span> <span className="k">{hardware.backends.map((b) => b.toUpperCase()).join(' · ')}</span></div>
                </div>
              </div>
            )}

            {persona === 'dev' && (
              <div className="card">
                <div className="cardh"><Icon name="box" /><h2>Models</h2>
                  <span className="tag">{done} of {tiers.length || '—'} ready</span></div>
                {tiers.length === 0 && <div className="cli d">Sizing models to your machine…</div>}
                {tiers.map((t) => (
                  <div className="lrow" key={t.tier}>
                    <span className={`tier${t.tier === 'best' ? ' b' : ''}`}>{t.label}</span>
                    <div className="nm">
                      <b>{t.rec.display_name}</b>
                      <span>{modelBlurb(t.rec)}</span>
                    </div>
                    <span className="sz">{downloadGb(t.rec)}G</span>
                    <span className="mini"><i className="a" style={{ width: `${t.status === 'ready' ? 100 : t.pct}%` }} /></span>
                    <span className="act" style={{ width: 58, textAlign: 'right' }}>
                      {t.status === 'ready' ? 'Ready'
                        : t.status === 'error' ? 'Failed'
                          : t.status === 'downloading' ? `${Math.round(t.pct)}%` : 'Queued'}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="go">
            {persona === 'dev' && (
              <button type="button" onClick={onBrowseAll}>Browse all models</button>
            )}
            {canSkip && <button type="button" onClick={onContinue}>Continue anyway</button>}
            <span className="note">
              {quickFailed
                ? `Couldn't download the first model${quick?.detail ? `: ${quick.detail}` : '.'}`
                : stalled
                  ? 'This is taking a while. Large models can be slow — the download keeps going in the background.'
                  : 'Everything runs on this machine.'}
            </span>
          </div>
        </>
      )}
    </div>
  );
}

/** A compact 1-10 score with a filled bar. Used across onboarding + the model library. */
export function ScoreBar({ label, score }: { label: string; score: number }) {
  const pct = Math.max(0, Math.min(100, score * 10));
  return (
    <div>
      <div className="flex items-center justify-between text-[10px] text-text-muted mb-1">
        <span>{label}</span>
        <span className="text-text font-medium">{score.toFixed(1)}/10</span>
      </div>
      <div className="h-1 bg-white/[0.06] rounded-full overflow-hidden">
        <div className="h-full bg-accent rounded-full" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
