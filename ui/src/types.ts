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

export interface Recommendation {
  model_id: string;
  display_name: string;
  params_b: number;
  quality_score: number;
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
