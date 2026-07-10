import { useState, useEffect, useCallback, useRef } from 'react'
import type { Recommendation, Tier, CustomModelInput, DownloadProgress } from '../types'
import { invoke, listen, isTauri } from '../tauri'
import { ScoreBar } from './SetupWizard'

// ── Plain-language tier labels (no jargon) ─────────────────
const TIER_LABEL: Record<Tier, string> = {
  wont_run: "Won't run here",
  slow: 'Runs slowly',
  okay: 'Runs okay',
  great: 'Runs great',
  blazing: 'Blazing fast',
};

function tierClasses(tier: Tier): string {
  switch (tier) {
    case 'blazing':
    case 'great':
      return 'text-success bg-success/10';
    case 'okay':
      return 'text-info bg-info/10';
    case 'slow':
      return 'text-warning bg-warning/10';
    case 'wont_run':
      return 'text-error bg-error/10';
  }
}

// Shown in a plain browser dev build (no Tauri backend).
const MOCK_RECS: Recommendation[] = [
  {
    model_id: 'llama3.2-3b', display_name: 'Llama 3.2 3B', params_b: 3.2,
    quality_score: 63, intelligence_score: 6.3, speed_score: 10, quant: 'Q8_0',
    tier: 'blazing', estimated_tokens_per_sec: 120, measured_tokens_per_sec: null,
    memory_fit: { required_mb: 3521, gpu_available_mb: 8192, ram_available_mb: 31372, fits_gpu: true, offload: false, gpu_layers_fraction: 1 },
    context_comfortable: 131072, why: 'Fits fully in VRAM.', ollama_pull: 'llama3.2:3b',
  },
  {
    model_id: 'qwen2.5-7b', display_name: 'Qwen 2.5 7B', params_b: 7,
    quality_score: 78, intelligence_score: 7.8, speed_score: 7, quant: 'Q4_K_M',
    tier: 'great', estimated_tokens_per_sec: 55, measured_tokens_per_sec: null,
    memory_fit: { required_mb: 4800, gpu_available_mb: 8192, ram_available_mb: 31372, fits_gpu: true, offload: false, gpu_layers_fraction: 1 },
    context_comfortable: 32768, why: 'Great all-rounder.', ollama_pull: 'qwen2.5:7b',
  },
  {
    model_id: 'llama3.1-70b', display_name: 'Llama 3.1 70B', params_b: 70,
    quality_score: 92, intelligence_score: 9.2, speed_score: 2, quant: 'Q4_K_M',
    tier: 'slow', estimated_tokens_per_sec: 4, measured_tokens_per_sec: null,
    memory_fit: { required_mb: 42000, gpu_available_mb: 8192, ram_available_mb: 31372, fits_gpu: false, offload: true, gpu_layers_fraction: 0.2 },
    context_comfortable: 8192, why: 'Very smart but heavy.', ollama_pull: 'llama3.1:70b',
  },
];

const CUSTOM_TAGS_KEY = 'fitllm.customTags';

function readCustomTags(): Set<string> {
  try {
    const raw = localStorage.getItem(CUSTOM_TAGS_KEY);
    if (raw) return new Set(JSON.parse(raw) as string[]);
  } catch { /* ignore */ }
  return new Set();
}

function writeCustomTags(tags: Set<string>) {
  try {
    localStorage.setItem(CUSTOM_TAGS_KEY, JSON.stringify([...tags]));
  } catch { /* ignore */ }
}

type SortKey = 'intelligence' | 'speed';

export function ModelLibrary() {
  const [models, setModels] = useState<Recommendation[]>([]);
  const [loading, setLoading] = useState(true);
  const [sortKey, setSortKey] = useState<SortKey>('intelligence');
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});
  const [customTags, setCustomTags] = useState<Set<string>>(() => readCustomTags());
  const [showAdd, setShowAdd] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    const recs = await invoke<Recommendation[]>('get_recommendations');
    setModels(recs && recs.length ? recs : MOCK_RECS);
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);

  // Live download progress from the backend.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<DownloadProgress>('download_progress', (p) => {
      setProgress((prev) => ({ ...prev, [p.tag]: p }));
    }).then((u) => { unlisten = u; });
    return () => { unlisten?.(); };
  }, []);

  const sorted = [...models].sort((a, b) =>
    sortKey === 'intelligence'
      ? b.intelligence_score - a.intelligence_score
      : b.speed_score - a.speed_score
  );

  async function handleDownload(tag: string) {
    setProgress((prev) => ({ ...prev, [tag]: { tag, pct: 0, status: 'starting', detail: 'Preparing…' } }));
    if (isTauri()) {
      await invoke('download_model', { tag });
    } else {
      // Simulate progress so the dev build shows the bar working.
      simulateDownload(tag, setProgress);
    }
  }

  async function handleAdd(input: CustomModelInput) {
    await invoke('add_custom_model', { ...input });
    const next = new Set(customTags);
    next.add(input.ollama_pull);
    setCustomTags(next);
    writeCustomTags(next);
    setShowAdd(false);
    if (isTauri()) {
      await load();
    } else {
      // Reflect the addition locally in the dev build.
      setModels((prev) => [
        ...prev,
        {
          model_id: `custom-${input.ollama_pull}`,
          display_name: input.display_name,
          params_b: input.params_b,
          quality_score: input.quality_score ?? 50,
          intelligence_score: input.quality_score ? input.quality_score / 10 : 5,
          speed_score: Math.max(1, Math.min(10, Math.round(10 - input.params_b / 8))),
          quant: 'Q4_K_M', tier: 'okay',
          estimated_tokens_per_sec: null, measured_tokens_per_sec: null,
          memory_fit: { required_mb: input.params_b * 700, gpu_available_mb: 0, ram_available_mb: 0, fits_gpu: false, offload: false, gpu_layers_fraction: 0 },
          context_comfortable: input.context_default ?? 8192,
          why: 'Added by you.', ollama_pull: input.ollama_pull,
        },
      ]);
    }
  }

  async function handleRemove(rec: Recommendation) {
    await invoke('remove_custom_model', { id: rec.model_id });
    const next = new Set(customTags);
    next.delete(rec.ollama_pull);
    setCustomTags(next);
    writeCustomTags(next);
    if (isTauri()) await load();
    else setModels((prev) => prev.filter((m) => m.model_id !== rec.model_id));
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
      {/* Header + sort */}
      <div className="flex-shrink-0 px-4 pt-4 pb-3 border-b border-border">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-sm font-semibold text-text">Model library</h1>
            <p className="text-[11px] text-text-muted mt-0.5">
              Every model we can run on your machine. Higher bars are better.
            </p>
          </div>
          <button
            onClick={() => setShowAdd((s) => !s)}
            className="px-2.5 py-1.5 text-[12px] rounded border border-border text-text-secondary
                       hover:border-border-strong hover:text-text transition-colors"
          >
            {showAdd ? 'Close' : '+ Add a model'}
          </button>
        </div>

        <div className="flex items-center gap-2 mt-3">
          <span className="text-[10px] text-text-muted uppercase tracking-wide">Sort by</span>
          {(['intelligence', 'speed'] as SortKey[]).map((k) => (
            <button
              key={k}
              onClick={() => setSortKey(k)}
              className={`px-2 py-0.5 rounded text-[11px] capitalize transition-colors ${
                sortKey === k
                  ? 'bg-accent-dim text-accent'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
            >
              {k === 'intelligence' ? 'Smartest' : 'Fastest'}
            </button>
          ))}
        </div>
      </div>

      {showAdd && <AddModelForm onAdd={handleAdd} />}

      {/* List */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2">
        {loading && (
          <div className="text-[11px] text-text-muted text-center py-8">Loading models…</div>
        )}
        {!loading && sorted.length === 0 && (
          <div className="text-[11px] text-text-muted text-center py-8">
            No models yet. Use “Add a model” to add one.
          </div>
        )}
        {sorted.map((rec) => (
          <ModelRow
            key={rec.model_id}
            rec={rec}
            progress={progress[rec.ollama_pull]}
            isCustom={customTags.has(rec.ollama_pull)}
            onDownload={() => handleDownload(rec.ollama_pull)}
            onRemove={() => handleRemove(rec)}
          />
        ))}
      </div>
    </div>
  );
}

function ModelRow({
  rec, progress, isCustom, onDownload, onRemove,
}: {
  rec: Recommendation;
  progress?: DownloadProgress;
  isCustom: boolean;
  onDownload: () => void;
  onRemove: () => void;
}) {
  const downloading = !!progress && progress.pct < 100 && progress.status !== 'done';
  const done = progress?.pct === 100 || progress?.status === 'done';
  const sizeGb = rec.memory_fit.required_mb ? (rec.memory_fit.required_mb / 1024).toFixed(1) : null;

  return (
    <div className="border border-border rounded-lg p-3 bg-surface">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-[13px] font-medium text-text truncate">{rec.display_name}</span>
            {isCustom && (
              <span className="text-[9px] text-text-muted border border-border rounded px-1 py-px">
                yours
              </span>
            )}
          </div>
          <div className="flex items-center gap-2 mt-1">
            <span className={`text-[10px] rounded px-1.5 py-0.5 font-medium ${tierClasses(rec.tier)}`}>
              {TIER_LABEL[rec.tier]}
            </span>
            {sizeGb && <span className="text-[10px] text-text-muted">{sizeGb} GB download</span>}
          </div>
        </div>

        <div className="flex items-center gap-1.5 flex-shrink-0">
          {!downloading && !done && (
            <button
              onClick={onDownload}
              className="px-3 py-1.5 rounded text-[12px] font-medium bg-accent text-white
                         hover:opacity-90 transition-opacity"
            >
              Download
            </button>
          )}
          {done && (
            <span className="text-[11px] text-success font-medium px-2">Installed</span>
          )}
          {isCustom && (
            <button
              onClick={onRemove}
              title="Remove this model"
              className="text-text-muted hover:text-error p-1 transition-colors"
            >
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" />
              </svg>
            </button>
          )}
        </div>
      </div>

      {/* Scores side by side */}
      <div className="grid grid-cols-2 gap-3 mt-3">
        <ScoreBar label="Intelligence" score={rec.intelligence_score} />
        <ScoreBar label="Speed" score={rec.speed_score} />
      </div>

      {/* Live download progress */}
      {downloading && (
        <div className="mt-3 space-y-1">
          <div className="h-1 bg-white/[0.06] rounded-full overflow-hidden">
            <div className="h-full bg-accent rounded-full transition-all duration-300"
                 style={{ width: `${progress?.pct ?? 0}%` }} />
          </div>
          <div className="flex justify-between text-[10px] text-text-muted">
            <span>{progress?.detail || progress?.status || 'Downloading…'}</span>
            <span>{Math.round(progress?.pct ?? 0)}%</span>
          </div>
        </div>
      )}
    </div>
  );
}

function AddModelForm({ onAdd }: { onAdd: (input: CustomModelInput) => void }) {
  const [name, setName] = useState('');
  const [tag, setTag] = useState('');
  const [size, setSize] = useState('');
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => { nameRef.current?.focus(); }, []);

  const sizeNum = parseFloat(size);
  const valid = name.trim() && tag.trim() && !isNaN(sizeNum) && sizeNum > 0;

  function submit() {
    if (!valid) return;
    onAdd({ display_name: name.trim(), ollama_pull: tag.trim(), params_b: sizeNum });
    setName(''); setTag(''); setSize('');
  }

  return (
    <div className="flex-shrink-0 px-4 py-3 border-b border-border bg-surface/50 space-y-3">
      <p className="text-[11px] text-text-muted">
        Know a model you want to try? Add it here and we'll fetch it for you.
      </p>
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <Field label="Name" hint="What you'll see in the list">
          <input
            ref={nameRef}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="My favorite model"
            className="w-full bg-bg border border-border rounded px-2 py-1.5 text-[12px] text-text
                       placeholder:text-text-muted focus:border-accent outline-none"
          />
        </Field>
        <Field label="Download tag" hint="The name to pull, e.g. mistral:7b">
          <input
            value={tag}
            onChange={(e) => setTag(e.target.value)}
            placeholder="mistral:7b"
            className="w-full bg-bg border border-border rounded px-2 py-1.5 text-[12px] text-text
                       placeholder:text-text-muted focus:border-accent outline-none font-mono"
          />
        </Field>
        <Field label="Rough size" hint="In billions (e.g. 7)">
          <input
            value={size}
            onChange={(e) => setSize(e.target.value)}
            inputMode="decimal"
            placeholder="7"
            className="w-full bg-bg border border-border rounded px-2 py-1.5 text-[12px] text-text
                       placeholder:text-text-muted focus:border-accent outline-none"
          />
        </Field>
      </div>
      <button
        onClick={submit}
        disabled={!valid}
        className={`px-3 py-1.5 rounded text-[12px] font-medium transition-colors ${
          valid ? 'bg-accent text-white hover:opacity-90'
                : 'bg-white/[0.04] text-text-muted border border-border cursor-not-allowed'
        }`}
      >
        Add model
      </button>
    </div>
  );
}

function Field({ label, hint, children }: { label: string; hint: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="text-[11px] text-text font-medium">{label}</span>
      <span className="block text-[10px] text-text-muted mb-1">{hint}</span>
      {children}
    </label>
  );
}

// Fake progress for the browser dev build only.
function simulateDownload(
  tag: string,
  setProgress: React.Dispatch<React.SetStateAction<Record<string, DownloadProgress>>>
) {
  let pct = 0;
  const iv = setInterval(() => {
    pct = Math.min(100, pct + Math.random() * 18);
    const done = pct >= 100;
    setProgress((prev) => ({
      ...prev,
      [tag]: { tag, pct, status: done ? 'done' : 'downloading', detail: done ? 'Installed' : 'Downloading…' },
    }));
    if (done) clearInterval(iv);
  }, 350);
}
