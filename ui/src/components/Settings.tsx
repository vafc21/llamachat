import { useState, useEffect } from 'react'
import type { AppSettings, ModelCatalog, CatalogModel, BenchmarkIntensity, HardwareProfile, TierModel } from '../types'
import { ReadinessChecklist } from './Readiness'
import { PersonaCards } from './PersonaChoice'
import { Icon } from './Icon'
import { INTENSITY_OPTIONS } from '../types'
import { invoke, isTauri } from '../tauri'
import type { Persona } from '../persona'
import type { Platform } from '../platform'
import type { UiPrefs } from '../prefs'

const INTENSITY_KEY = 'llamachat.benchmarkIntensity'

function defaultSettings(hardware: HardwareProfile | null): AppSettings {
  let intensity: BenchmarkIntensity = 'balanced';
  try {
    const saved = localStorage.getItem(INTENSITY_KEY) as BenchmarkIntensity | null;
    if (saved) intensity = saved;
  } catch { /* ignore */ }
  return {
    benchmark_intensity: intensity,
    model_override: null,
    models_dir: hardware?.storage.models_dir ?? null,
    memory_dir: null,
    perception: 'accessibility',
    vision_model: null,
    telemetry_off: true,
  };
}

interface Props {
  hardware: HardwareProfile | null;
  persona: Persona;
  onPersona: (p: Persona) => void;
  platform: Platform;
  prefs: UiPrefs;
  onPrefs: (p: UiPrefs) => void;
  tiers: TierModel[];
}

/** A v6 `.srow` with a toggle on the right. */
function Toggle({ title, blurb, on, onToggle }: { title: string; blurb: string; on: boolean; onToggle: () => void }) {
  return (
    <div className="srow">
      <div className="tx"><b>{title}</b><span>{blurb}</span></div>
      <button
        type="button"
        className={`tog${on ? ' on' : ''}`}
        role="switch"
        aria-checked={on}
        aria-label={title}
        onClick={onToggle}
      >
        <i />
      </button>
    </div>
  );
}

/**
 * Settings — restructured to v6's `.setwrap`.
 *
 * The persona choice lives at the top (R15): the same two cards as the startup
 * question, changeable at any time. Everything below is gated by persona, so a
 * simple user never sees a runtime knob and a developer never sees the
 * "let LlamaChat decide" copy aimed at people who don't want to know.
 */
export function Settings({ hardware, persona, onPersona, platform, prefs, onPrefs, tiers }: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [catalog, setCatalog] = useState<CatalogModel[]>([]);
  const [memoryDir, setMemoryDir] = useState('');
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    async function load() {
      const s = await invoke<AppSettings>('get_settings');
      setSettings(s ?? defaultSettings(hardware));
      const cat = await invoke<ModelCatalog>('get_catalog');
      setCatalog(cat?.models ?? []);
      setMemoryDir((await invoke<string>('get_memory_dir')) ?? '');
    }
    load();
  }, [hardware]);

  async function update(patch: Partial<AppSettings>) {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    try { localStorage.setItem(INTENSITY_KEY, next.benchmark_intensity); } catch { /* ignore */ }
    await invoke('set_settings', { settings: next });
    setSaved(true);
    window.setTimeout(() => setSaved(false), 1500);
  }

  function togglePref(k: keyof UiPrefs) {
    onPrefs({ ...prefs, [k]: !prefs[k] });
  }

  if (!settings) {
    return (
      <div className="setwrap"><div className="setinner">
        <span style={{ fontSize: 12, color: 'var(--text3)' }}>Loading settings…</span>
      </div></div>
    );
  }

  const catalogOptions = catalog.length > 0
    ? catalog.map((m) => ({ value: m.ollama_pull ?? m.model_id, label: m.display_name }))
    : tiers.map((t) => ({ value: t.rec.ollama_pull, label: t.rec.display_name }));

  return (
    <div className="setwrap">
      <div className="setinner">
        <h1>Settings{saved && <span style={{ fontSize: 11.5, color: 'var(--ok)', marginLeft: 12, fontWeight: 400 }}>Saved</span>}</h1>

        {/* R15 — persona, changeable any time. */}
        <div className="sgroup">
          <h2>How you use LlamaChat</h2>
          <PersonaCards persona={persona} onPick={onPersona} />
        </div>

        {/* Simple persona knobs — plain words only, no model names (R17). */}
        <div className="sgroup simple-only">
          <h2>Answers</h2>
          <Toggle
            title="Let LlamaChat decide"
            blurb="Picks the best model on this machine for each message, and how long to think about it."
            on={prefs.autoRoute}
            onToggle={() => togglePref('autoRoute')}
          />
          <Toggle
            title="Prefer speed over depth"
            blurb="Answer faster, think less. Good on battery."
            on={prefs.preferSpeed}
            onToggle={() => togglePref('preferSpeed')}
          />
        </div>

        {/* Developer knobs (R16, R19). */}
        <div className="sgroup dev-only">
          <h2>Runtime</h2>
          <Toggle
            title="Show the status bar"
            blurb="Tokens/sec, context, CPU, GPU and VRAM along the bottom of the window."
            on={prefs.statusBar}
            onToggle={() => togglePref('statusBar')}
          />
          <Toggle
            title="Explain router decisions"
            blurb="Show which model was chosen for each reply and why."
            on={prefs.explainRouter}
            onToggle={() => togglePref('explainRouter')}
          />
          <div className="srow">
            <div className="tx">
              <b>Default model</b>
              <span>Used when Auto is off.</span>
            </div>
            <select
              value={settings.model_override ?? ''}
              onChange={(e) => update({ model_override: e.target.value || null })}
              aria-label="Default model"
            >
              <option value="">Auto (recommended)</option>
              {catalogOptions.map((o) => (
                <option key={o.value} value={o.value}>{o.label}</option>
              ))}
            </select>
          </div>
        </div>

        {/* Benchmarking depth — developer-facing detail. */}
        <div className="sgroup dev-only">
          <h2>How hard to test</h2>
          {INTENSITY_OPTIONS.map((opt) => {
            const active = settings.benchmark_intensity === opt.id;
            return (
              <button
                key={opt.id}
                type="button"
                className="srow"
                style={{ width: '100%', textAlign: 'left', borderColor: active ? 'rgba(77,124,255,.5)' : undefined }}
                onClick={() => update({ benchmark_intensity: opt.id })}
              >
                <div className="tx">
                  <b>{opt.title} <span style={{ color: 'var(--text3)', fontWeight: 400 }}>· {opt.blurb}</span></b>
                  <span>{opt.detail}</span>
                </div>
                {active && <Icon name="check" size={15} />}
              </button>
            );
          })}
        </div>

        {/* Storage. */}
        <div className="sgroup">
          <h2>Storage</h2>
          <div className="srow">
            <div className="tx">
              <b>Models</b>
              <span style={{ fontFamily: 'var(--mono)' }}>{settings.models_dir ?? 'Not set'}</span>
            </div>
          </div>
          <div className="srow">
            <div className="tx">
              <b>Chats &amp; memory</b>
              <span>Saved as editable markdown. Currently: <span style={{ fontFamily: 'var(--mono)' }}>{memoryDir || '—'}</span></span>
            </div>
            <input
              type="text"
              value={settings.memory_dir ?? ''}
              onChange={(e) => update({ memory_dir: e.target.value || null })}
              onBlur={async () => setMemoryDir((await invoke<string>('get_memory_dir')) ?? '')}
              placeholder="Default app data folder"
              aria-label="Chats and memory folder"
              style={{ width: 220 }}
            />
          </div>
        </div>

        {/* Agent abilities. */}
        <div className="sgroup">
          <h2>Agent abilities</h2>
          {/* Same checklist component as the startup readiness step, so the two
              can never disagree about what is granted. `showVision` is on here
              and off at startup: a 4.7 GB pull is a settings decision, not a
              first-run gate. */}
          <ReadinessChecklist persona={persona} platform={platform} showVision />

          {/* Perception is a developer choice: "accessibility tree vs
              screenshot vision" is exactly the jargon R17's persona split
              exists to keep away from the simple side. */}
          <div className="srow dev-only">
            <div className="tx">
              <b>How the agent sees your screen</b>
              <span>The accessibility tree is faster and works with text models; screenshot vision is more general but needs a vision model.</span>
            </div>
            <select
              value={settings.perception || 'accessibility'}
              onChange={(e) => update({ perception: e.target.value })}
              aria-label="Agent perception"
            >
              <option value="accessibility">Accessibility tree</option>
              <option value="vision">Screenshot vision</option>
            </select>
          </div>
          {settings.perception === 'vision' && (
            <div className="srow dev-only">
              <div className="tx">
                <b>Vision model</b>
                <span>Describes screenshots to the agent.</span>
              </div>
              <input
                type="text"
                value={settings.vision_model ?? ''}
                onChange={(e) => update({ vision_model: e.target.value || null })}
                placeholder="llava:7b"
                aria-label="Vision model"
                style={{ width: 200 }}
              />
            </div>
          )}
        </div>

        {/* Privacy. */}
        <div className="sgroup">
          <h2>Privacy</h2>
          <div className="srow">
            <div className="tx">
              <b>Everything stays on this machine</b>
              <span>
                No account, no telemetry, no network calls. Models run through your local runtime.
                {!isTauri() && ' (Browser dev build — the desktop backend isn\u2019t attached.)'}
              </span>
            </div>
            <span style={{ fontSize: 11.5, color: 'var(--ok)' }}>
              {settings.telemetry_off ? 'Enforced' : 'On'}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
