import { useState, useEffect } from 'react'
import type { HardwareProfile, Recommendation, BenchmarkIntensity } from '../types'
import { INTENSITY_OPTIONS } from '../types'

const INTENSITY_KEY = 'fitllm.benchmarkIntensity'

// Mock hardware data — real version calls the Tauri backend
const MOCK_HARDWARE: HardwareProfile = {
  cpu: {
    model: 'AMD Ryzen 5 5500',
    vendor: 'AuthenticAMD',
    physical_cores: 6,
    logical_cores: 12,
    max_clock_mhz: 4507,
    flags: { avx2: true, avx512: false, fma: true, f16c: true, neon: false },
  },
  gpus: [
    {
      vendor: 'NVIDIA',
      model: 'GeForce GTX 1080',
      vram_total_mb: 8192,
      vram_free_mb: 6305,
      backend: 'cuda',
      cuda_version: '13.0',
    },
  ],
  memory: { total_mb: 31372, available_mb: 23512 },
  storage: { models_dir: '~/.cache/fitllm/models', free_mb: 839995 },
  os: { name: 'Ubuntu', version: '26.04', arch: 'x86_64' },
  backends: ['cuda', 'cpu'],
  detected_at: new Date().toISOString(),
};

const MOCK_RECS: Recommendation[] = [
  {
    model_id: 'llama3.2-3b',
    display_name: 'Llama 3.2 3B',
    params_b: 3.2,
    quality_score: 63,
    intelligence_score: 6.3,
    speed_score: 10,
    quant: 'Q8_0',
    tier: 'blazing',
    estimated_tokens_per_sec: 120,
    measured_tokens_per_sec: null,
    memory_fit: {
      required_mb: 3521,
      gpu_available_mb: 8192,
      ram_available_mb: 31372,
      fits_gpu: true,
      offload: false,
      gpu_layers_fraction: 1.0,
    },
    context_comfortable: 131072,
    why: 'Blazing: ~120 tok/s on GPU, fits fully in 8GB VRAM, 128k context.',
    ollama_pull: 'llama3.2:3b',
  },
];

type Step = 'profiling' | 'intensity' | 'recommendation' | 'downloading' | 'done';

interface Props {
  onComplete: (hw: HardwareProfile, model: string) => void;
}

export function SetupWizard({ onComplete }: Props) {
  const [step, setStep] = useState<Step>('profiling');
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  const [rec, setRec] = useState<Recommendation | null>(null);
  const [progress, setProgress] = useState(0);
  const [eta, setEta] = useState('');
  const [intensity, setIntensity] = useState<BenchmarkIntensity>('balanced');

  // Simulate hardware detection, then ask how hard to benchmark.
  useEffect(() => {
    if (step !== 'profiling') return;
    const timer = setTimeout(() => {
      setHardware(MOCK_HARDWARE);
      setRec(MOCK_RECS[0]);
      setStep('intensity');
    }, 1200);
    return () => clearTimeout(timer);
  }, [step]);

  function confirmIntensity() {
    try {
      localStorage.setItem(INTENSITY_KEY, intensity);
    } catch { /* storage may be unavailable; the default still applies */ }
    setStep('recommendation');
  }

  // Simulate download
  function handleDownload() {
    if (!rec) return;
    setStep('downloading');
    setProgress(0);

    const sizeMB = rec.memory_fit.required_mb;
    const totalMs = Math.min(sizeMB * 2, 15000); // simulate network speed

    const start = Date.now();
    const iv = setInterval(() => {
      const elapsed = Date.now() - start;
      const pct = Math.min(Math.round((elapsed / totalMs) * 100), 99);
      setProgress(pct);

      const remaining = totalMs - elapsed;
      if (remaining > 0) {
        const secs = Math.ceil(remaining / 1000);
        setEta(`${secs}s remaining`);
      }
    }, 200);

    setTimeout(() => {
      clearInterval(iv);
      setProgress(100);
      setEta('');
      setTimeout(() => setStep('done'), 500);
    }, totalMs);
  }

  return (
    <div className="h-full flex items-center justify-center bg-bg">
      <div className="w-full max-w-md">
        {/* Step 1: Profiling */}
        {step === 'profiling' && (
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

        {/* Step 1.5: Benchmark intensity */}
        {step === 'intensity' && (
          <div className="space-y-5">
            <div>
              <p className="text-sm text-text font-medium">How hard should we test?</p>
              <p className="text-[11px] text-text-muted mt-1">
                This is how thoroughly FitLLM measures models on your machine. You can change it later in Settings.
              </p>
            </div>

            <div className="space-y-2">
              {INTENSITY_OPTIONS.map((opt) => {
                const active = intensity === opt.id;
                return (
                  <button
                    key={opt.id}
                    onClick={() => setIntensity(opt.id)}
                    className={`w-full text-left rounded-lg p-3 border transition-colors ${
                      active
                        ? 'border-accent bg-accent-dim'
                        : 'border-border bg-surface hover:border-accent/40'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-[13px] font-medium text-text">
                        {opt.title} <span className="text-text-muted font-normal">· {opt.blurb}</span>
                      </span>
                      <span
                        className={`w-3.5 h-3.5 rounded-full border ${
                          active ? 'border-accent bg-accent' : 'border-text-muted'
                        }`}
                      />
                    </div>
                    <p className="text-[11px] text-text-muted mt-1">{opt.detail}</p>
                  </button>
                );
              })}
            </div>

            <button
              onClick={confirmIntensity}
              className="w-full py-2.5 bg-accent text-white text-[13px] font-medium rounded-lg
                         hover:opacity-90 transition-opacity"
            >
              Continue
            </button>
          </div>
        )}

        {/* Step 2: Recommendation */}
        {step === 'recommendation' && hardware && rec && (
          <div className="space-y-5">
            {/* Hardware summary */}
            <div className="border border-border rounded-lg p-4 bg-surface">
              <div className="text-[10px] text-text-muted uppercase tracking-wide mb-3">
                Your machine
              </div>
              <div className="grid grid-cols-2 gap-2 text-[12px]">
                <Row label="CPU" value={`${hardware.cpu.model} (${hardware.cpu.physical_cores}C/${hardware.cpu.logical_cores}T)`} />
                <Row label="GPU" value={`${hardware.gpus[0].model} · ${(hardware.gpus[0].vram_total_mb! / 1024).toFixed(0)}GB`} />
                <Row label="RAM" value={`${(hardware.memory.total_mb / 1024).toFixed(0)}GB (${(hardware.memory.available_mb / 1024).toFixed(0)}GB free)`} />
                <Row label="Backend" value={hardware.backends.map(b => b.toUpperCase()).join(' · ')} />
              </div>
            </div>

            {/* Recommendation */}
            <div className="border border-accent/30 rounded-lg p-4 bg-accent-dim">
              <div className="flex items-center justify-between mb-2">
                <span className="text-[10px] text-text-muted uppercase tracking-wide">
                  Best model for your machine
                </span>
                <span className="text-[10px] text-accent font-medium">
                  Blazing
                </span>
              </div>
              <div className="text-sm font-medium text-text mb-1">
                {rec.display_name} · {rec.quant}
              </div>
              <p className="text-[11px] text-text-secondary leading-relaxed">
                {rec.why}
              </p>
              <div className="mt-3 grid grid-cols-2 gap-3">
                <ScoreBar label="Intelligence" score={rec.intelligence_score} />
                <ScoreBar label="Speed" score={rec.speed_score} />
              </div>
              <div className="flex gap-3 mt-3 text-[10px] text-text-muted">
                <span>{rec.memory_fit.required_mb}MB download</span>
                <span>~{rec.estimated_tokens_per_sec?.toFixed(0)} tok/s</span>
              </div>
            </div>

            <button
              onClick={handleDownload}
              className="w-full py-2.5 bg-accent text-white text-[13px] font-medium rounded-lg
                         hover:opacity-90 transition-opacity"
            >
              Download & Start
            </button>

            <p className="text-[10px] text-text-muted text-center">
              Downloads ~{rec.memory_fit.required_mb}MB. Nothing leaves your device.
            </p>
          </div>
        )}

        {/* Step 3: Downloading */}
        {step === 'downloading' && (
          <div className="space-y-4">
            <div>
              <p className="text-sm text-text font-medium">Downloading model</p>
              <p className="text-[11px] text-text-muted mt-0.5">
                {rec?.display_name} · {rec?.quant} · {rec?.ollama_pull}
              </p>
            </div>

            {/* Progress bar */}
            <div className="space-y-1.5">
              <div className="h-1 bg-white/[0.04] rounded-full overflow-hidden">
                <div
                  className="h-full bg-accent rounded-full transition-all duration-300"
                  style={{ width: `${progress}%` }}
                />
              </div>
              <div className="flex justify-between text-[10px] text-text-muted">
                <span>{progress}%</span>
                <span>{eta}</span>
              </div>
            </div>
          </div>
        )}

        {/* Step 4: Done — transitions to chat */}
        {step === 'done' && (
          <div className="text-center space-y-4 animate-fade-in">
            <div className="w-8 h-8 rounded-full bg-accent-dim flex items-center justify-center mx-auto">
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <path d="M3 8l3 3 7-7" stroke="var(--color-accent)" strokeWidth="2" />
              </svg>
            </div>
            <div>
              <p className="text-sm text-text font-medium">Ready</p>
              <p className="text-[11px] text-text-muted mt-1">
                {rec?.display_name} is loaded and ready.
              </p>
            </div>
            <button
              onClick={() => hardware && rec && onComplete(hardware, rec.ollama_pull)}
              className="px-6 py-2 bg-accent text-white text-[13px] font-medium rounded-lg
                         hover:opacity-90 transition-opacity"
            >
              Start
            </button>
          </div>
        )}
      </div>
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
