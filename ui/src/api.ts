/**
 * Mock API layer for FitLLM. When running inside Tauri, these call
 * `window.__TAURI__.invoke(...)`. When running standalone (`npm run dev`),
 * they return realistic sample data so the UI can be developed independently.
 */

// ── Type mirrors (same shapes as Rust types.rs) ──────────────────────────

export interface CpuFlags {
  avx2: boolean;
  avx512: boolean;
  fma: boolean;
  f16c: boolean;
  neon: boolean;
}

export interface Cpu {
  model: string;
  vendor: string;
  physical_cores: number;
  logical_cores: number;
  base_clock_mhz: number | null;
  max_clock_mhz: number | null;
  flags: CpuFlags;
}

export interface Gpu {
  vendor: string;
  model: string;
  vram_total_mb: number | null;
  vram_free_mb: number | null;
  driver_version: string | null;
  cuda_version: string | null;
  compute_capability: string | null;
  backend: string;
  is_integrated: boolean;
}

export interface AppleSilicon {
  unified_memory: boolean;
  gpu_cores: number | null;
  neural_engine: boolean;
  chip: string;
}

export interface Memory {
  total_mb: number;
  available_mb: number;
}

export interface Storage {
  models_dir: string;
  free_mb: number;
  read_mbps: number | null;
}

export interface Os {
  name: string;
  version: string;
  arch: string;
}

export interface HardwareProfile {
  cpu: Cpu;
  gpus: Gpu[];
  apple_silicon: AppleSilicon | null;
  memory: Memory;
  storage: Storage;
  os: Os;
  backends: string[];
  detected_at: string;
}

export interface Quant {
  name: string;
  bits: number;
  size_mb: number;
  ollama_tag: string | null;
}

export interface CatalogModel {
  id: string;
  family: string;
  display_name: string;
  params_b: number;
  license: string;
  quality_score: number;
  quality_source: string;
  context_default: number;
  context_max: number;
  quants: Quant[];
  ollama_pull: string;
  tags: string[];
}

export interface ModelCatalog {
  schema_version: number;
  updated_at: string;
  models: CatalogModel[];
  frontier: FrontierModel[];
}

export interface FrontierModel {
  id: string;
  display_name: string;
  provider: string;
  quality_score: number;
  quality_source: string;
  typical_tps: number | null;
}

export interface MemoryFit {
  required_mb: number;
  gpu_available_mb: number;
  ram_available_mb: number;
  fits_gpu: boolean;
  fits_ram: boolean;
  offload: boolean;
  gpu_layers_fraction: number;
}

export type Tier = 'wont_run' | 'slow' | 'okay' | 'great' | 'blazing';
export type RatingSource = 'heuristic' | 'measured';

export interface Recommendation {
  model_id: string;
  display_name: string;
  family: string;
  params_b: number;
  quality_score: number;
  quant: string;
  tier: Tier;
  source: RatingSource;
  estimated_tokens_per_sec: number | null;
  measured_tokens_per_sec: number | null;
  ttft_ms: number | null;
  memory_fit: MemoryFit;
  context_comfortable: number;
  why: string;
  ollama_pull: string;
}

// ── Tauri bridge detection ──────────────────────────────────────────────

declare global {
  interface Window {
    __TAURI__?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  }
}

function isTauri(): boolean {
  return typeof window.__TAURI__ !== 'undefined';
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error('Not in Tauri');
  return window.__TAURI__!.invoke(cmd, args) as Promise<T>;
}

// ── Mock data ────────────────────────────────────────────────────────────

const MOCK_HARDWARE: HardwareProfile = {
  cpu: {
    model: 'AMD Ryzen 5 5500',
    vendor: 'AuthenticAMD',
    physical_cores: 6,
    logical_cores: 12,
    base_clock_mhz: 3600,
    max_clock_mhz: 4507,
    flags: { avx2: true, avx512: false, fma: true, f16c: true, neon: false },
  },
  gpus: [
    {
      vendor: 'NVIDIA',
      model: 'GeForce GTX 1080',
      vram_total_mb: 8192,
      vram_free_mb: 7500,
      driver_version: '580.159.03',
      cuda_version: '13.0',
      compute_capability: '6.1',
      backend: 'cuda',
      is_integrated: false,
    },
  ],
  apple_silicon: null,
  memory: { total_mb: 32768, available_mb: 24000 },
  storage: {
    models_dir: '~/.cache/fitllm/models',
    free_mb: 860000,
    read_mbps: null,
  },
  os: { name: 'Ubuntu', version: '24.04', arch: 'x86_64' },
  backends: ['cuda', 'cpu'],
  detected_at: new Date().toISOString(),
};

const MOCK_RECOMMENDATIONS: Recommendation[] = [
  {
    model_id: 'mistral-7b',
    display_name: 'Mistral 7B',
    family: 'Mistral',
    params_b: 7.3,
    quality_score: 58,
    quant: 'Q8_0',
    tier: 'great',
    source: 'heuristic',
    estimated_tokens_per_sec: 55,
    measured_tokens_per_sec: null,
    ttft_ms: null,
    memory_fit: {
      required_mb: 7300,
      gpu_available_mb: 8192,
      ram_available_mb: 24000,
      fits_gpu: true,
      fits_ram: true,
      offload: false,
      gpu_layers_fraction: 1.0,
    },
    context_comfortable: 8192,
    why: 'Great: estimated 55 tok/s, fits fully in 8GB VRAM, 8k context comfortable',
    ollama_pull: 'mistral:7b',
  },
  {
    model_id: 'llama3.2-3b',
    display_name: 'Llama 3.2 3B',
    family: 'Llama',
    params_b: 3.2,
    quality_score: 54,
    quant: 'Q8_0',
    tier: 'blazing',
    source: 'heuristic',
    estimated_tokens_per_sec: 80,
    measured_tokens_per_sec: null,
    ttft_ms: null,
    memory_fit: {
      required_mb: 3200,
      gpu_available_mb: 8192,
      ram_available_mb: 24000,
      fits_gpu: true,
      fits_ram: true,
      offload: false,
      gpu_layers_fraction: 1.0,
    },
    context_comfortable: 16384,
    why: 'Blazing: estimated 80 tok/s, massive VRAM headroom, 16k context comfortable',
    ollama_pull: 'llama3.2:3b',
  },
  {
    model_id: 'llama3.1-8b',
    display_name: 'Llama 3.1 8B',
    family: 'Llama',
    params_b: 8.0,
    quality_score: 64,
    quant: 'Q8_0',
    tier: 'great',
    source: 'heuristic',
    estimated_tokens_per_sec: 50,
    measured_tokens_per_sec: null,
    ttft_ms: null,
    memory_fit: {
      required_mb: 8000,
      gpu_available_mb: 8192,
      ram_available_mb: 24000,
      fits_gpu: true,
      fits_ram: true,
      offload: false,
      gpu_layers_fraction: 1.0,
    },
    context_comfortable: 4096,
    why: 'Great: estimated 50 tok/s, tight VRAM fit (8GB), 4k context comfortable',
    ollama_pull: 'llama3.1:8b',
  },
  {
    model_id: 'phi3-mini',
    display_name: 'Phi-3 Mini',
    family: 'Phi',
    params_b: 3.8,
    quality_score: 56,
    quant: 'Q8_0',
    tier: 'blazing',
    source: 'heuristic',
    estimated_tokens_per_sec: 75,
    measured_tokens_per_sec: null,
    ttft_ms: null,
    memory_fit: {
      required_mb: 3800,
      gpu_available_mb: 8192,
      ram_available_mb: 24000,
      fits_gpu: true,
      fits_ram: true,
      offload: false,
      gpu_layers_fraction: 1.0,
    },
    context_comfortable: 8192,
    why: 'Blazing: estimated 75 tok/s, 4.4GB VRAM headroom, 8k context comfortable',
    ollama_pull: 'phi3:mini',
  },
  {
    model_id: 'qwen2.5-32b',
    display_name: 'Qwen 2.5 32B',
    family: 'Qwen',
    params_b: 32.5,
    quality_score: 72,
    quant: 'Q4_K_M',
    tier: 'slow',
    source: 'heuristic',
    estimated_tokens_per_sec: 8,
    measured_tokens_per_sec: null,
    ttft_ms: null,
    memory_fit: {
      required_mb: 17875,
      gpu_available_mb: 8192,
      ram_available_mb: 24000,
      fits_gpu: false,
      fits_ram: true,
      offload: true,
      gpu_layers_fraction: 0.35,
    },
    context_comfortable: 2048,
    why: 'Slow: estimated 8 tok/s, 35% layers on GPU, heavy CPU offload, 2k context comfortable',
    ollama_pull: 'qwen2.5:32b',
  },
  {
    model_id: 'llama3.1-70b',
    display_name: 'Llama 3.1 70B',
    family: 'Llama',
    params_b: 70.6,
    quality_score: 78,
    quant: 'Q4_K_M',
    tier: 'wont_run',
    source: 'heuristic',
    estimated_tokens_per_sec: null,
    measured_tokens_per_sec: null,
    ttft_ms: null,
    memory_fit: {
      required_mb: 38830,
      gpu_available_mb: 8192,
      ram_available_mb: 24000,
      fits_gpu: false,
      fits_ram: false,
      offload: true,
      gpu_layers_fraction: 0.0,
    },
    context_comfortable: 0,
    why: "Won't run: needs ~39GB at Q4_K_M, only 24GB RAM available",
    ollama_pull: 'llama3.1:70b',
  },
];

// ── Public API ───────────────────────────────────────────────────────────

export async function getHardwareProfile(): Promise<HardwareProfile> {
  if (isTauri()) return tauriInvoke<HardwareProfile>('get_hardware_profile');
  // Simulate a tiny delay for realistic loading state
  await new Promise((r) => setTimeout(r, 400));
  return { ...MOCK_HARDWARE, detected_at: new Date().toISOString() };
}

export async function getRecommendations(): Promise<Recommendation[]> {
  if (isTauri()) return tauriInvoke<Recommendation[]>('get_recommendations');
  await new Promise((r) => setTimeout(r, 600));
  return MOCK_RECOMMENDATIONS;
}

export async function startQuickBenchmark(): Promise<void> {
  if (isTauri()) {
    await tauriInvoke('start_quick_benchmark');
    return;
  }
  // Mock: the real one emits events; we simulate nothing
}

export async function getCatalog(): Promise<ModelCatalog> {
  if (isTauri()) return tauriInvoke<ModelCatalog>('get_catalog');
  await new Promise((r) => setTimeout(r, 300));
  return {
    schema_version: 1,
    updated_at: '2026-07-09T00:00:00Z',
    models: [],
    frontier: [],
  };
}

export async function setConsent(granted: boolean): Promise<void> {
  if (isTauri()) {
    await tauriInvoke('set_consent', { granted });
    return;
  }
  localStorage.setItem('fitllm-consent', JSON.stringify(granted));
}

export async function getConsent(): Promise<boolean> {
  if (isTauri()) return tauriInvoke<boolean>('get_consent');
  return JSON.parse(localStorage.getItem('fitllm-consent') || 'false');
}
