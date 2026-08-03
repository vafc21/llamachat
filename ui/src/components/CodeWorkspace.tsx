import type { HardwareProfile, TierModel, Recommendation, Tier } from '../types'
import type { ActivityEntry } from '../runtime'
import { hardwareReadouts, runtimeLine } from '../runtime'
import { downloadGb } from '../models'
import { Icon, type IconName } from './Icon'

interface Props {
  greeting: string;
  hardware: HardwareProfile | null;
  tiers: TierModel[];
  selectedModel: string;
  onUseModel: (tag: string) => void;
  onManageLibrary: () => void;
  /** Real tool calls from the agent loop, newest last. */
  activity: ActivityEntry[];
  ctxUsed: number;
  ctxTotal: number;
}

/**
 * The Code workspace (R5–R10). Deliberately NOT a code editor:
 *
 *   R8 — it renders no source. It is a GUI over the `llama-cli` surface: what
 *        loaded, how it was offloaded, what the tools did.
 *   R9 — it surfaces the model library inline.
 *   R6 — CPU / RAM / GPU / VRAM readouts sit right here.
 *   R7 — every marker is a line glyph. No emoji anywhere on this screen.
 */
export function CodeWorkspace({
  greeting, hardware, tiers, selectedModel, onUseModel, onManageLibrary,
  activity, ctxUsed, ctxTotal,
}: Props) {
  const current = tiers.find((t) => t.rec.ollama_pull === selectedModel);
  const rec = current?.rec ?? null;
  const readouts = hardwareReadouts(hardware);

  return (
    <div className="codetop">
      <div className="chead">
        {/* R11 — the greeting survives into Code mode, sans-serif like v6/ref3. */}
        <h1>{greeting}</h1>
        <span>{runtimeLine(rec, hardware) || 'no model loaded'}</span>
      </div>

      {/* ── Load: the llama-cli view of what's running (R8) ─────────── */}
      <div className="card">
        <div className="cardh">
          <Icon name="term" /><h2>Load</h2>
          <span className="tag">{current?.status === 'ready' ? 'ready' : (current?.status ?? 'idle')}</span>
        </div>
        <div className="cli">
          {rec ? <LoadLines rec={rec} hw={hardware} /> : <div className="d">No model selected.</div>}
        </div>
      </div>

      {/* ── Machine: CPU / RAM / GPU / VRAM (R6) ────────────────────── */}
      <div className="card">
        <div className="cardh">
          <Icon name="cpu" /><h2>Machine</h2>
          <span className="tag">{hardware?.os.name ?? 'unknown'} {hardware?.os.arch ?? ''}</span>
        </div>
        <div className="log">
          {readouts.map((r) => (
            <div className="lg" key={r.label}>
              <Icon name={READOUT_ICON[r.label] ?? 'cpu'} />
              <span className="p" style={{ flex: 'none', width: 52, color: 'var(--text3)' }}>{r.label}</span>
              <span className="mini">
                <i
                  className={r.label === 'VRAM' ? 'w' : r.label === 'GPU' ? 'a' : ''}
                  style={{ width: `${r.pct ?? 0}%` }}
                />
              </span>
              <span className="p">{r.detail}</span>
              <span className="t">{r.pct === null ? 'n/a' : `${Math.round(r.pct)}%`}</span>
            </div>
          ))}
        </div>
      </div>

      {/* ── Context window (R5) ─────────────────────────────────────── */}
      <div className="card">
        <div className="cardh">
          <Icon name="box" /><h2>Context window</h2>
          <span className="tag">{ctxTotal ? `${Math.round((ctxUsed / ctxTotal) * 100)}%` : '—'}</span>
        </div>
        <div className="log">
          <div className="lg">
            <Icon name="dash" />
            <span className="mini" style={{ width: 160 }}>
              <i className="a" style={{ width: `${ctxTotal ? Math.min(100, (ctxUsed / ctxTotal) * 100) : 0}%` }} />
            </span>
            <span className="p">~{ctxUsed.toLocaleString()} of {ctxTotal.toLocaleString()} tokens in this conversation</span>
            <span className="t">estimated</span>
          </div>
        </div>
      </div>

      {/* ── Library (R9) ────────────────────────────────────────────── */}
      <div className="card">
        <div className="cardh">
          <Icon name="box" /><h2>Library</h2>
          <button type="button" className="act" onClick={onManageLibrary}>Manage</button>
        </div>
        {tiers.length === 0 && <div className="cli d">Sizing models to this machine…</div>}
        {tiers.map((t) => {
          const active = t.rec.ollama_pull === selectedModel;
          const ready = t.status === 'ready';
          return (
            <div className="lrow" key={t.tier}>
              <span className={`tier${t.tier === 'best' ? ' b' : ''}`}>{t.label}</span>
              <div className="nm">
                <b>{t.rec.display_name}</b>
                <span>{t.rec.ollama_pull} · {t.rec.quant}</span>
              </div>
              <span className={`rate ${RATE_CLASS[t.rec.tier]}`}>
                <i />{RATE_LABEL[t.rec.tier]}
              </span>
              <span className="sz">{downloadGb(t.rec)}G</span>
              <button
                type="button"
                className={`act${active ? ' on' : ''}`}
                disabled={!ready || active}
                onClick={() => onUseModel(t.rec.ollama_pull)}
              >
                {active ? 'Loaded' : ready ? 'Load' : t.status === 'downloading' ? `${Math.round(t.pct)}%` : '—'}
              </button>
            </div>
          );
        })}
      </div>

      {/* ── Activity: real tool calls from the agent loop ───────────── */}
      <div className="card">
        <div className="cardh">
          <Icon name="tool" /><h2>Activity</h2>
          <span className="tag">tools armed</span>
        </div>
        <div className="log">
          {activity.length === 0 && (
            <div className="lg"><Icon name="dash" /><span className="p">No tool calls yet in this session.</span></div>
          )}
          {activity.slice(-12).map((a) => (
            <div className={`lg ${a.state === 'ok' ? 'ok' : a.state === 'error' ? 'err' : 'run'}`} key={a.id}>
              <Icon name={a.state === 'running' ? 'run' : a.state === 'ok' ? 'check' : 'x'} />
              <span className="p">{a.tool.padEnd(6)} {a.detail}</span>
              <span className="t">
                {a.endedAt ? `${((a.endedAt - a.startedAt) / 1000).toFixed(1)}s` : 'running'}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

const READOUT_ICON: Record<string, IconName> = {
  CPU: 'cpu', RAM: 'ram', GPU: 'gpu', VRAM: 'gpu',
};

const RATE_LABEL: Record<Tier, string> = {
  blazing: 'Blazing', great: 'Great', okay: 'Okay', slow: 'Slow', wont_run: "Won't run",
};
const RATE_CLASS: Record<Tier, string> = {
  blazing: 'blazing', great: 'great', okay: 'okay', slow: 'slow', wont_run: 'no',
};

/**
 * The llama.cpp-style load report, built from the recommender's real numbers
 * rather than a canned transcript. Fields with no backend source are omitted
 * instead of being invented.
 */
function LoadLines({ rec, hw }: { rec: Recommendation; hw: HardwareProfile | null }) {
  const fit = rec.memory_fit;
  const layerPct = Math.round(fit.gpu_layers_fraction * 100);
  const flags = hw
    ? [
        hw.cpu.flags.avx512 && 'AVX512',
        hw.cpu.flags.avx2 && 'AVX2',
        hw.cpu.flags.neon && 'NEON',
        hw.cpu.flags.fma && 'FMA',
      ].filter(Boolean).join(' · ')
    : '';
  const tps = rec.measured_tokens_per_sec ?? rec.estimated_tokens_per_sec;

  return (
    <>
      <div>
        <span className="d">model:</span> <span className="k">{rec.ollama_pull}</span> ·{' '}
        <span className="k">{rec.quant}</span> · {rec.params_b}B params
      </div>
      <div>
        <span className="d">offload:</span>{' '}
        {fit.fits_gpu
          ? <><span className="k">all</span> layers → GPU</>
          : fit.gpu_layers_fraction > 0
            ? <><span className="k">{layerPct}%</span> of layers → GPU, remainder on CPU</>
            : <>CPU only</>}
        {' · '}<span className="k">{(fit.required_mb / 1024).toFixed(2)} GiB</span> weights
      </div>
      <div>
        <span className="d">context:</span> n_ctx <span className="k">{rec.context_comfortable.toLocaleString()}</span>
        {fit.gpu_available_mb > 0 && <> · VRAM available <span className="k">{(fit.gpu_available_mb / 1024).toFixed(1)} GiB</span></>}
      </div>
      {hw && (
        <div>
          <span className="d">system:</span> {hw.cpu.model} · {hw.cpu.logical_cores} threads
          {flags && <> · {flags}</>}
          {hw.backends.length > 0 && <> · {hw.backends.map((b) => b.toUpperCase()).join(' ')}</>}
        </div>
      )}
      <div>
        {tps
          ? <>
              <span className="g">ready</span>{' '}
              <span className="d">
                · {rec.measured_tokens_per_sec ? 'measured' : 'estimated'} {tps.toFixed(1)} tok/s
              </span>
            </>
          : <span className="d">not benchmarked yet</span>}
      </div>
    </>
  );
}
