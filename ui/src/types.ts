// ── Types shared between UI components ─────────────────────

export interface Cpu {
  model: string;
  vendor: string;
  physical_cores: number;
  logical_cores: number;
  max_clock_mhz: number | null;
  flags: { avx2: boolean; avx512: boolean; fma: boolean; f16c: boolean; neon: boolean };
}

export interface Gpu {
  vendor: string;
  model: string;
  vram_total_mb: number | null;
  vram_free_mb: number | null;
  backend: string;
  cuda_version: string | null;
}

export interface HardwareProfile {
  cpu: Cpu;
  gpus: Gpu[];
  memory: { total_mb: number; available_mb: number };
  storage: { models_dir: string; free_mb: number };
  os: { name: string; version: string; arch: string };
  backends: string[];
  detected_at: string;
}

export type Tier = 'wont_run' | 'slow' | 'okay' | 'great' | 'blazing';

/** How hard FitLLM benchmarks — chosen on first run, changeable in Settings. */
export type BenchmarkIntensity = 'quick' | 'balanced' | 'full';

export interface IntensityOption {
  id: BenchmarkIntensity;
  title: string;
  blurb: string;
  detail: string;
}

export const INTENSITY_OPTIONS: IntensityOption[] = [
  {
    id: 'quick',
    title: 'Quick',
    blurb: 'A fast check',
    detail: 'About 30 seconds. Great to get started right away.',
  },
  {
    id: 'balanced',
    title: 'Balanced',
    blurb: 'A solid test',
    detail: 'A few minutes. The recommended default for most people.',
  },
  {
    id: 'full',
    title: 'Full',
    blurb: 'The works',
    detail: 'Thorough and most accurate. Can take a while to finish.',
  },
];

export interface Recommendation {
  model_id: string;
  display_name: string;
  params_b: number;
  quality_score: number;
  /** "How smart" on a 1-10 scale, derived from quality_score. */
  intelligence_score: number;
  /** "How fast it runs on this machine" on a 1-10 scale. */
  speed_score: number;
  quant: string;
  tier: Tier;
  estimated_tokens_per_sec: number | null;
  measured_tokens_per_sec: number | null;
  memory_fit: {
    required_mb: number;
    gpu_available_mb: number;
    ram_available_mb: number;
    fits_gpu: boolean;
    offload: boolean;
    gpu_layers_fraction: number;
  };
  context_comfortable: number;
  why: string;
  ollama_pull: string;
}

// ── Model library + settings ───────────────────────────────

/** User-tunable app settings, persisted by the backend. */
export interface AppSettings {
  benchmark_intensity: BenchmarkIntensity;
  /** ollama tag to always use, or null to auto-pick the best model. */
  model_override: string | null;
  /** Where downloaded models live on disk (display only). */
  models_dir: string | null;
  /** True when usage reporting is turned off (it always is — shown for reassurance). */
  telemetry_off: boolean;
}

/** Fields the user fills in when adding their own model. */
export interface CustomModelInput {
  display_name: string;
  ollama_pull: string;
  params_b: number;
  quality_score?: number;
  context_default?: number;
}

/** One entry in the browsable model catalog. */
export interface CatalogModel {
  model_id: string;
  display_name: string;
  params_b?: number;
  ollama_pull?: string;
}

/** The full catalog returned by get_catalog(). */
export interface ModelCatalog {
  models: CatalogModel[];
}

/** Payload of a "download_progress" event. */
export interface DownloadProgress {
  tag: string;
  pct: number;
  status: string;
  detail?: string;
}

export interface ToolCall {
  name: string;
  args: Record<string, unknown>;
  result?: string;
  error?: string;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  timestamp: string;
  toolCall?: ToolCall;
  streaming?: boolean;
}

export interface Conversation {
  id: string;
  title: string;
  messages: Message[];
  createdAt: string;
}
