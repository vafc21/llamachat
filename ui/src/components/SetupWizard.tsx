import type { HardwareProfile, TierModel } from '../types'
import { downloadGb, modelBlurb } from '../models'

interface Props {
  /** 'profiling' while detecting hardware, 'setup' while pulling models. */
  phase: 'profiling' | 'setup';
  hardware: HardwareProfile | null;
  tiers: TierModel[];
  /** Continue to chat manually (offered if the Quick model fails to download). */
  onContinue: () => void;
  /** Open the full model catalog (all AIs) to pick or add a custom one. */
  onBrowseAll: () => void;
}

/**
 * First-run onboarding — purely presentational. App.tsx drives the real work
 * (hardware detection via `get_hardware_profile`, auto-downloading the three
 * tier models) and feeds the live state in as props. Chat opens automatically
 * the moment the Quick model is ready.
 */
export function SetupWizard({ phase, hardware, tiers, onContinue, onBrowseAll }: Props) {
  const quick = tiers[0];
  const quickFailed = quick?.status === 'error';
  const totalGb = tiers.reduce((sum, t) => sum + (parseFloat(downloadGb(t.rec)) || 0), 0);

  return (
    <div className="h-full flex items-center justify-center bg-bg">
      <div className="w-full max-w-md">
        {phase === 'profiling' && (
          <div className="text-center space-y-4">
            <div className="w-8 h-8 border-2 border-accent border-t-transparent rounded-full animate-spin mx-auto" />
            <div>
              <p className="text-sm text-text font-medium">Profiling your hardware</p>
              <p className="text-[11px] text-text-muted mt-1">
                Reading CPU, GPU, RAM, and storage — nothing leaves your device.
              </p>
            </div>
          </div>
        )}

        {phase === 'setup' && (
          <div className="space-y-5">
            {/* Hardware summary — real, detected values */}
            {hardware && (
              <div className="border border-border rounded-lg p-4 bg-surface">
                <div className="text-[10px] text-text-muted uppercase tracking-wide mb-3">Your machine</div>
                <div className="grid grid-cols-2 gap-2 text-[12px]">
                  <Row label="CPU" value={`${hardware.cpu.model} (${hardware.cpu.physical_cores}C/${hardware.cpu.logical_cores}T)`} />
                  <Row
                    label="GPU"
                    value={
                      hardware.gpus[0]
                        ? `${hardware.gpus[0].model}${hardware.gpus[0].vram_total_mb ? ` · ${(hardware.gpus[0].vram_total_mb / 1024).toFixed(0)}GB` : ''}`
                        : 'Integrated'
                    }
                  />
                  <Row label="RAM" value={`${(hardware.memory.total_mb / 1024).toFixed(0)}GB (${(hardware.memory.available_mb / 1024).toFixed(0)}GB free)`} />
                  <Row label="Backend" value={hardware.backends.map((b) => b.toUpperCase()).join(' · ')} />
                </div>
              </div>
            )}

            {/* Auto-download of the three tiers */}
            <div>
              <p className="text-sm text-text font-medium">Setting up your models</p>
              <p className="text-[11px] text-text-muted mt-1">
                Downloading a Quick, Smart, and Best model sized to your machine
                {totalGb > 0 ? ` (~${totalGb.toFixed(1)} GB total)` : ''}. Chat opens as soon as Quick is
                ready — the rest finish in the background.
              </p>
            </div>

            <div className="space-y-2">
              {tiers.map((t) => (
                <DownloadRow key={t.tier} t={t} />
              ))}
              {tiers.length === 0 && (
                <p className="text-[11px] text-text-muted italic">Sizing models to your machine…</p>
              )}
            </div>

            {/* Custom: browse the entire catalog of AIs */}
            <button
              onClick={onBrowseAll}
              className="w-full py-2 text-[12px] text-text-secondary rounded-lg border border-border
                         hover:border-border-strong hover:text-text transition-colors"
            >
              Custom — browse all models &amp; pick your own →
            </button>

            {quickFailed && (
              <div className="space-y-2">
                <p className="text-[11px] text-red-400">
                  Couldn't download the Quick model{quick?.detail ? `: ${quick.detail}` : '.'}
                </p>
                <button
                  onClick={onContinue}
                  className="w-full py-2 bg-accent text-white text-[13px] font-medium rounded-lg hover:opacity-90 transition-opacity"
                >
                  Continue to chat anyway
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/** One tier's card: what the model is, how big, its scores, and download state. */
function DownloadRow({ t }: { t: TierModel }) {
  const done = t.status === 'ready';
  const failed = t.status === 'error';
  return (
    <div className="border border-border rounded-lg p-3 bg-surface">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-[12px] text-text font-medium flex items-center gap-1.5">
            <span>{t.icon}</span> {t.label}
            <span className="text-text-muted font-normal">· {t.rec.display_name}</span>
          </div>
          {/* Explain the model */}
          <p className="text-[11px] text-text-muted mt-0.5 leading-snug">{modelBlurb(t.rec)}</p>
        </div>
        <span className={`text-[10px] flex-shrink-0 ${done ? 'text-emerald-400' : failed ? 'text-red-400' : 'text-text-muted'}`}>
          {done ? 'Ready' : failed ? 'Failed' : t.status === 'downloading' ? `${Math.round(t.pct)}%` : 'Queued'}
        </span>
      </div>

      {/* Size + scores */}
      <div className="flex items-center gap-3 mt-2 text-[10px] text-text-muted">
        <span className="text-text">{downloadGb(t.rec)} GB download</span>
        <span>smart {t.rec.intelligence_score.toFixed(0)}/10</span>
        <span>fast {t.rec.speed_score.toFixed(0)}/10</span>
      </div>

      {!done && !failed && (
        <div className="mt-2 h-1 bg-white/[0.04] rounded-full overflow-hidden">
          <div className="h-full bg-accent rounded-full transition-all duration-300" style={{ width: `${t.pct}%` }} />
        </div>
      )}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="overflow-hidden">
      <div className="text-[10px] text-text-muted">{label}</div>
      <div className="text-[11px] text-text truncate" title={value}>{value}</div>
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
