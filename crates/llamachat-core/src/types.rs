//! Shared data types for LlamaChat. These are the interface contract between the
//! hardware profiler, the model catalog, the recommendation engine, the Python
//! benchmark sidecar, and the UI. All types serialize to the JSON shapes
//! documented in `CONTRACT.md` — keep them in sync.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Hardware profile
// ---------------------------------------------------------------------------

/// A normalized snapshot of the machine, produced by the hardware profiler and
/// read by everything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu: Cpu,
    pub gpus: Vec<Gpu>,
    /// Present only on Apple Silicon; `None` elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apple_silicon: Option<AppleSilicon>,
    pub memory: Memory,
    pub storage: Storage,
    pub os: Os,
    /// Acceleration backends available on this machine, e.g. ["cuda", "cpu"].
    pub backends: Vec<String>,
    /// ISO-8601 timestamp of when this profile was captured.
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cpu {
    pub model: String,
    pub vendor: String,
    pub physical_cores: u32,
    pub logical_cores: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_clock_mhz: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_clock_mhz: Option<f64>,
    pub flags: CpuFlags,
}

/// Instruction-set features that matter for LLM inference throughput.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuFlags {
    pub avx2: bool,
    pub avx512: bool,
    pub fma: bool,
    pub f16c: bool,
    /// ARM NEON (Apple Silicon / ARM servers).
    pub neon: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gpu {
    pub vendor: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_total_mb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_free_mb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_capability: Option<String>,
    /// Best inference backend for this GPU: "cuda", "rocm", "metal", "vulkan".
    pub backend: String,
    /// True for iGPUs / integrated graphics that share system RAM.
    pub is_integrated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleSilicon {
    /// Apple Silicon uses unified memory shared between CPU and GPU — this
    /// changes model-size math versus discrete VRAM.
    pub unified_memory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_cores: Option<u32>,
    pub neural_engine: bool,
    pub chip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub total_mb: u64,
    pub available_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    /// Directory where models are (or would be) stored.
    pub models_dir: String,
    pub free_mb: u64,
    /// Sequential read speed if cheaply measurable; matters for weight load time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_mbps: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Os {
    pub name: String,
    pub version: String,
    pub arch: String,
}

// ---------------------------------------------------------------------------
// Model catalog
// ---------------------------------------------------------------------------

/// The bundled, updatable catalog of open models plus frontier cloud references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub schema_version: u32,
    pub updated_at: String,
    pub models: Vec<CatalogModel>,
    /// Frontier hosted models for the (Phase 2) cloud-comparison panel. Kept in
    /// the catalog so the reference list can be refreshed without a code change.
    #[serde(default)]
    pub frontier: Vec<FrontierModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub family: String,
    pub display_name: String,
    pub params_b: f64,
    pub license: String,
    /// A public, approximate quality score (0-100), higher is better.
    pub quality_score: f64,
    pub quality_source: String,
    pub context_default: u32,
    pub context_max: u32,
    pub quants: Vec<Quant>,
    /// Default Ollama pull tag for the recommended quant.
    pub ollama_pull: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quant {
    /// Quant name, e.g. "Q4_K_M", "Q8_0", "FP16".
    pub name: String,
    /// Effective bits-per-weight.
    pub bits: f64,
    /// On-disk / in-memory weight footprint in MB.
    pub size_mb: u64,
    /// Ollama tag that pulls this specific quant, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierModel {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub quality_score: f64,
    pub quality_source: String,
    /// Rough typical output speed for the hosted model (tokens/sec), for the
    /// speed side-by-side. Approximate and clearly labeled in the UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typical_tps: Option<f64>,
}

// ---------------------------------------------------------------------------
// Benchmark results (produced by the Python sidecar)
// ---------------------------------------------------------------------------

/// A single benchmark run for one model on one adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub model: String,
    pub adapter: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_tps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gen_tps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_mem_mb: Option<f64>,
    pub context_tested: u32,
    /// System load level (0.0-1.0) at run time, so results are comparable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_load: Option<f64>,
    /// "quick" or "full".
    pub tier: String,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Recommendation output
// ---------------------------------------------------------------------------

/// The 5-tier rating for a model on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Not enough VRAM/RAM even at the smallest quant.
    WontRun,
    /// Works, but under the usable-speed threshold or heavy swapping.
    Slow,
    /// Usable interactive speed.
    Okay,
    /// Comfortable headroom, fast.
    Great,
    /// Best-in-class for this machine.
    Blazing,
}

impl Tier {
    pub fn label(&self) -> &'static str {
        match self {
            Tier::WontRun => "Won't run",
            Tier::Slow => "Runs, but slow",
            Tier::Okay => "Runs okay",
            Tier::Great => "Runs great",
            Tier::Blazing => "Blazing",
        }
    }
    /// Ordering rank for sorting (higher is better).
    pub fn rank(&self) -> u8 {
        match self {
            Tier::WontRun => 0,
            Tier::Slow => 1,
            Tier::Okay => 2,
            Tier::Great => 3,
            Tier::Blazing => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RatingSource {
    /// Spec heuristic (provisional, shown instantly).
    Heuristic,
    /// Backed by a real on-device benchmark.
    Measured,
}

/// How a model's weights map onto this machine's memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFit {
    pub required_mb: u64,
    /// GPU VRAM available (sum across usable GPUs / unified memory).
    pub gpu_available_mb: u64,
    pub ram_available_mb: u64,
    pub fits_gpu: bool,
    pub fits_ram: bool,
    /// True if the model must partially offload to CPU/RAM to fit.
    pub offload: bool,
    /// Fraction of layers that fit on the GPU (0.0-1.0).
    pub gpu_layers_fraction: f64,
}

/// A per-model recommendation for this machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub model_id: String,
    pub display_name: String,
    pub family: String,
    pub params_b: f64,
    pub quality_score: f64,
    /// Plain-language "how smart" rating on a 1-10 scale, derived from
    /// `quality_score`. Shown next to every model so non-experts can compare.
    pub intelligence_score: f64,
    /// Plain-language "how fast it runs on *this* machine" rating on a 1-10
    /// scale, derived from the measured (or estimated) tokens/sec.
    pub speed_score: f64,
    /// The quant the engine picked as the best fit.
    pub quant: String,
    pub tier: Tier,
    pub source: RatingSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_tokens_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured_tokens_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<f64>,
    pub memory_fit: MemoryFit,
    pub context_comfortable: u32,
    /// Human-readable explanation of the rating.
    pub why: String,
    /// Ollama tag to pull/run this model.
    pub ollama_pull: String,
}

// ---------------------------------------------------------------------------
// Claude-Code-style permission modes + effort levels (shared by CLI and desktop)
// ---------------------------------------------------------------------------

/// Claude-Code-style permission mode. Controls whether the agent asks before
/// editing files or running shell commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermMode {
    /// Ask before every destructive tool call (Claude Code's Manual/default).
    Manual,
    /// Auto-approve file edits + common filesystem commands (mkdir, touch, mv, cp).
    AcceptEdits,
    /// Reads only — all mutating tools are denied before they run.
    Plan,
    /// Everything auto-approved with safety checks.
    Auto,
    /// Everything auto-approved, no checks. For isolated containers.
    Bypass,
}

impl PermMode {
    pub const ALL: [PermMode; 5] = [
        PermMode::Manual,
        PermMode::AcceptEdits,
        PermMode::Plan,
        PermMode::Auto,
        PermMode::Bypass,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            PermMode::Manual => "manual",
            PermMode::AcceptEdits => "accept-edits",
            PermMode::Plan => "plan",
            PermMode::Auto => "auto",
            PermMode::Bypass => "bypass",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            PermMode::Manual => "⏸ manual",
            PermMode::AcceptEdits => "✎ accept-edits",
            PermMode::Plan => "◎ plan",
            PermMode::Auto => "▶ auto",
            PermMode::Bypass => "⚠ bypass",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        match s {
            "manual" | "default" => Some(PermMode::Manual),
            "accept-edits" | "acceptedits" => Some(PermMode::AcceptEdits),
            "plan" => Some(PermMode::Plan),
            "auto" => Some(PermMode::Auto),
            "bypass" | "bypassPermissions" => Some(PermMode::Bypass),
            _ => None,
        }
    }

    /// Next mode in the Shift+Tab cycle.
    pub fn next(&self) -> Self {
        let idx = PermMode::ALL.iter().position(|m| m == self).unwrap_or(0);
        PermMode::ALL[(idx + 1) % PermMode::ALL.len()]
    }

    /// One-line explanation for UI display.
    pub fn explain(&self) -> &'static str {
        match self {
            PermMode::Manual => "asks before shell, file writes, and process",
            PermMode::AcceptEdits => "auto-approves file edits + mkdir/touch/mv/cp",
            PermMode::Plan => "read-only — model can read but not write or run commands",
            PermMode::Auto => "everything auto-approved",
            PermMode::Bypass => "all tools allowed, no prompts",
        }
    }

    /// In AcceptEdits mode, is this shell command safe to auto-approve?
    pub fn is_safe_cmd(cmd: &str) -> bool {
        let c = cmd.trim();
        if c.is_empty() {
            return false;
        }
        let name = c.split_whitespace().next().unwrap_or("");
        let safe = [
            "ls", "cat", "head", "tail", "wc", "grep", "find", "which", "echo",
            "pwd", "whoami", "date", "uname", "env", "printenv", "df", "du", "free",
            "uptime", "ps", "pgrep", "top", "mkdir", "touch", "mv", "cp", "ln",
            "chmod", "chown", "git", "cargo", "npm", "pip", "python", "python3", "node",
        ];
        safe.contains(&name)
    }
}

/// How hard the model should think before answering (Claude Code's /effort).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
    Max,
}

impl Effort {
    pub const ALL: [Effort; 4] = [Effort::Low, Effort::Medium, Effort::High, Effort::Max];

    pub fn label(&self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Max => "max",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Effort::Low => "effort:low",
            Effort::Medium => "effort:medium",
            Effort::High => "effort:high",
            Effort::Max => "effort:max",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Effort::Low),
            "medium" | "med" => Some(Effort::Medium),
            "high" => Some(Effort::High),
            "max" => Some(Effort::Max),
            _ => None,
        }
    }

    /// A short prompt prefix that hints the model how hard to think.
    pub fn system_hint(&self) -> &'static str {
        match self {
            Effort::Low => "Be concise. Give short, direct answers. Skip pleasantries.",
            Effort::Medium => "Think carefully. Provide thorough but focused answers.",
            Effort::High => "Reason step by step. Explore trade-offs, edge cases, and implications.",
            Effort::Max => "Think deeply about this problem. Consider multiple approaches, verify each step of your reasoning, anticipate follow-up questions, and give the most complete, well-structured answer you can.",
        }
    }
}

// ---------------------------------------------------------------------------

/// A concrete, hardware-sized plan of which model each benchmark *level* runs.
///
/// Levels are capability targets, not benchmark durations: each names the model
/// it will run — sized to this machine via the fit tiers — so the UI can show it
/// (name + intelligence/speed scores) BEFORE running, instead of a single opaque
/// global pick. See `docs/design/benchmark-levels.md`. Each field is `None` only
/// when nothing in the catalog runs on this machine at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelPlan {
    /// Quick: headline pick — best-quality model rated `Blazing` (else the fastest
    /// model that runs at all). The `*_set` fields below are what each tier
    /// actually runs and reports; these single picks are just the headline shown
    /// on the tier card.
    pub quick: Option<Recommendation>,
    /// Standard: headline pick — best-quality model rated `Great` or better.
    pub standard: Option<Recommendation>,
    /// Max: headline pick — best-quality model that runs at all (`Okay`+). On
    /// strong hardware this reaches a large model; never a tiny one when a bigger
    /// model fits.
    pub max: Option<Recommendation>,
    /// The whole runnable set (`Okay`+), best-first — what `Full`/`Max`/`All` run
    /// and report. Each setting benchmarks a *cohort* and reports every model, so
    /// the user can compare — it never picks one and stops.
    pub all: Vec<Recommendation>,
    /// Quick cohort: the fast models (rated `Blazing`), best-first. Falls back to
    /// the single fastest runnable model when nothing is `Blazing`.
    pub quick_set: Vec<Recommendation>,
    /// Standard cohort: models rated `Great` or better, best-first.
    pub standard_set: Vec<Recommendation>,
}
