import { useState, useEffect } from 'react'
import type { AppSettings, ModelCatalog, CatalogModel, BenchmarkIntensity, HardwareProfile } from '../types'
import { AgentSetup } from './AgentSetup'
import { INTENSITY_OPTIONS } from '../types'
import { invoke, isTauri } from '../tauri'

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
}

export function Settings({ hardware }: Props) {
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

  if (!settings) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <span className="text-[11px] text-text-muted">Loading settings…</span>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-xl mx-auto px-6 py-6 space-y-8">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-sm font-semibold text-text">Settings</h1>
            <p className="text-[11px] text-text-muted mt-0.5">
              Everything stays on this machine.
            </p>
          </div>
          {saved && <span className="text-[11px] text-success">Saved</span>}
        </div>

        {/* How hard to test */}
        <Section
          title="How hard should we test?"
          hint="How thoroughly LlamaChat measures models on your machine."
        >
          <div className="space-y-2">
            {INTENSITY_OPTIONS.map((opt) => {
              const active = settings.benchmark_intensity === opt.id;
              return (
                <button
                  key={opt.id}
                  onClick={() => update({ benchmark_intensity: opt.id })}
                  className={`w-full text-left rounded-lg p-3 border transition-colors ${
                    active ? 'border-accent bg-accent-dim' : 'border-border bg-surface hover:border-accent/40'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="text-[13px] font-medium text-text">
                      {opt.title} <span className="text-text-muted font-normal">· {opt.blurb}</span>
                    </span>
                    <span className={`w-3.5 h-3.5 rounded-full border ${
                      active ? 'border-accent bg-accent' : 'border-text-muted'
                    }`} />
                  </div>
                  <p className="text-[11px] text-text-muted mt-1">{opt.detail}</p>
                </button>
              );
            })}
          </div>
        </Section>

        {/* Which model to use */}
        <Section
          title="Which model should chats use?"
          hint="Pick a specific model, or let LlamaChat choose the best one for your machine."
        >
          <select
            value={settings.model_override ?? ''}
            onChange={(e) => update({ model_override: e.target.value || null })}
            className="w-full bg-bg border border-border rounded px-2 py-2 text-[12px] text-text
                       focus:border-accent outline-none"
          >
            <option value="">Auto (recommended)</option>
            {catalog.map((m) => (
              <option key={m.model_id} value={m.ollama_pull ?? m.model_id}>
                {m.display_name}
              </option>
            ))}
          </select>
          {catalog.length === 0 && (
            <p className="text-[10px] text-text-muted mt-1">
              {isTauri() ? 'No models in the catalog yet.' : 'Model list appears when running the desktop app.'}
            </p>
          )}
        </Section>

        {/* Where models live */}
        <Section title="Where models are stored" hint="Downloaded models are saved here.">
          <div className="bg-surface border border-border rounded px-3 py-2 text-[12px] text-text-secondary
                          font-mono truncate" title={settings.models_dir ?? ''}>
            {settings.models_dir ?? 'Not set'}
          </div>
        </Section>

        {/* Where chats & memory live */}
        <Section
          title="Where chats & memory are stored"
          hint="Your conversations and memory.md are saved here as editable markdown files."
        >
          <input
            value={settings.memory_dir ?? ''}
            onChange={(e) => update({ memory_dir: e.target.value || null })}
            onBlur={async () => setMemoryDir((await invoke<string>('get_memory_dir')) ?? '')}
            placeholder={memoryDir || 'Default app data folder'}
            className="w-full bg-bg border border-border rounded px-2 py-2 text-[12px] text-text
                       placeholder:text-text-muted focus:border-accent outline-none font-mono"
          />
          <p className="text-[10px] text-text-muted mt-1 font-mono truncate" title={memoryDir}>
            Currently: {memoryDir || '—'}
          </p>
        </Section>

        {/* Agent abilities & permissions */}
        <Section
          title="Agent abilities"
          hint="Enable these to let the agent control your Mac. Green ✓ means it's ready."
        >
          <AgentSetup />
        </Section>

        {/* Agent perception */}
        <Section
          title="How the agent sees your screen"
          hint="Agent mode needs to perceive the screen to click things. Text models work best with the accessibility tree."
        >
          <div className="space-y-2">
            {([
              { id: 'accessibility', title: 'Accessibility tree', desc: 'Reads on-screen elements as text + moves the real mouse. Fast, works with your text models.' },
              { id: 'vision', title: 'Screenshot vision', desc: 'A vision model describes a screenshot to the agent. More general, but needs a vision model and is slower.' },
            ]).map((opt) => {
              const active = (settings.perception || 'accessibility') === opt.id;
              return (
                <button
                  key={opt.id}
                  onClick={() => update({ perception: opt.id })}
                  className={`w-full text-left rounded-lg p-3 border transition-colors ${
                    active ? 'border-accent bg-accent-dim' : 'border-border bg-surface hover:border-accent/40'
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <span className={`w-3.5 h-3.5 rounded-full border ${active ? 'border-accent bg-accent' : 'border-text-muted'}`} />
                    <span className="text-[13px] font-medium text-text">{opt.title}</span>
                  </div>
                  <p className="text-[11px] text-text-muted mt-1 pl-5">{opt.desc}</p>
                </button>
              );
            })}
          </div>
          {settings.perception === 'vision' && (
            <div className="mt-2">
              <label className="text-[11px] text-text font-medium">Vision model</label>
              <input
                value={settings.vision_model ?? ''}
                onChange={(e) => update({ vision_model: e.target.value || null })}
                placeholder="llava:7b (default — auto-used when accessibility is empty)"
                className="w-full mt-1 bg-bg border border-border rounded px-2 py-2 text-[12px] text-text
                           placeholder:text-text-muted focus:border-accent outline-none font-mono"
              />
            </div>
          )}
          <p className="text-[10px] text-text-muted mt-2">
            Controlling the mouse/keyboard needs Accessibility permission (System Settings ▸ Privacy &amp; Security ▸ Accessibility); screenshots need Screen Recording.
          </p>
        </Section>

        {/* Privacy */}
        <Section title="Privacy" hint="LlamaChat never phones home.">
          <div className="flex items-center gap-2 bg-surface border border-border rounded px-3 py-2.5">
            <span className="w-2 h-2 rounded-full bg-success" />
            <span className="text-[12px] text-text">
              Usage reporting is {settings.telemetry_off ? 'off' : 'on'}
            </span>
            <span className="text-[11px] text-text-muted ml-auto">
              Nothing leaves your device
            </span>
          </div>
        </Section>
      </div>
    </div>
  );
}

function Section({ title, hint, children }: { title: string; hint: string; children: React.ReactNode }) {
  return (
    <section className="space-y-2">
      <div>
        <h2 className="text-[13px] font-medium text-text">{title}</h2>
        <p className="text-[11px] text-text-muted mt-0.5">{hint}</p>
      </div>
      {children}
    </section>
  );
}
