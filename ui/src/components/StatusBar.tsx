import type { HardwareProfile, TierModel } from '../types'
import { hardwareReadouts } from '../runtime'
import { Icon } from './Icon'
import { ContextRingMini } from './ContextRing'

interface Props {
  hardware: HardwareProfile | null;
  tiers: TierModel[];
  selectedModel: string;
  /** Last measured generation rate, or null before the first reply. */
  tokensPerSec: number | null;
  ctxUsed: number;
  ctxTotal: number;
}

/**
 * The developer status bar (R16, R19) — v6's `.status`, gated by `.dev-only` so
 * the simple persona never sees it (R17).
 *
 * Numbers with no live source render `n/a` rather than a plausible-looking
 * invention. See runtime.ts for exactly which those are (CPU load and GPU
 * utilisation, today).
 */
export function StatusBar({ hardware, tiers, selectedModel, tokensPerSec, ctxUsed, ctxTotal }: Props) {
  const current = tiers.find((t) => t.rec.ollama_pull === selectedModel);
  const [cpu, ram, gpu, vram] = hardwareReadouts(hardware);

  return (
    <div className="statusbar dev-only">
      <div className="st">
        <span className={current?.status === 'ready' ? 'dotok' : 'dotok'} style={current?.status === 'ready' ? undefined : { background: 'var(--warn)' }} />
        <b>{selectedModel}</b>
      </div>
      {current && <div className="st"><span className="lbl">{current.rec.quant}</span></div>}
      <div className="st"><b>{tokensPerSec !== null ? `${tokensPerSec.toFixed(1)} tok/s` : '— tok/s'}</b></div>

      <div className="sp st" />

      <div className="st" title={cpu.detail}>
        <Icon name="cpu" />
        <span className="mini"><i style={{ width: `${cpu.pct ?? 0}%` }} /></span>
        <b>{cpu.pct === null ? 'n/a' : `${Math.round(cpu.pct)}%`}</b>
      </div>
      <div className="st" title={ram.detail}>
        <Icon name="ram" />
        <span className="mini"><i style={{ width: `${ram.pct ?? 0}%` }} /></span>
        <b>{ram.detail}</b>
      </div>
      <div className="st" title={gpu.detail}>
        <Icon name="gpu" />
        <span className="mini"><i className="a" style={{ width: `${gpu.pct ?? 0}%` }} /></span>
        <b>{gpu.pct === null ? 'n/a' : `${Math.round(gpu.pct)}%`}</b>
      </div>
      <div className="st" title={vram.detail}>
        <span className="lbl">VRAM</span>
        <span className="mini"><i className="w" style={{ width: `${vram.pct ?? 0}%` }} /></span>
        <b>{vram.detail}</b>
      </div>
      <div className="st" style={{ borderRight: 'none' }} title="Estimated context use">
        <ContextRingMini used={ctxUsed} total={ctxTotal} />
        <b>{ctxUsed.toLocaleString()} / {ctxTotal.toLocaleString()}</b>
      </div>
    </div>
  );
}
