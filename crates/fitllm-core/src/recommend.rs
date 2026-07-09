//! Recommendation engine.
//!
//! Turns a [`HardwareProfile`] + [`ModelCatalog`] (+ optional measured
//! [`BenchmarkResult`]s) into a best-first list of [`Recommendation`]s.
//!
//! The math is intentionally simple and transparent — every rating comes with a
//! human-readable `why`. Heuristics are conservative: we would rather under-
//! promise speed than tell someone a model will fly when it will crawl.

use crate::types::*;

// ---------------------------------------------------------------------------
// Tunable thresholds (tokens/sec, generation). Exposed as module constants so
// the UI / tests can reference the same numbers.
// ---------------------------------------------------------------------------

/// Below this, a model that technically fits is rated [`Tier::Slow`].
pub const SLOW_MAX_TPS: f64 = 10.0;
/// Upper bound of the [`Tier::Okay`] band.
pub const OKAY_MAX_TPS: f64 = 25.0;
/// Upper bound of the [`Tier::Great`] band; at/above this we call it Blazing.
pub const GREAT_MAX_TPS: f64 = 50.0;

/// Fraction of memory kept free for the KV cache / runtime overhead.
const KV_HEADROOM: f64 = 0.10;
/// Headroom fraction above which the full `context_max` is considered comfortable.
const COMFORT_HEADROOM: f64 = 0.20;

/// Rate every model in the catalog for this machine, best-first.
pub fn rate_all(
    profile: &HardwareProfile,
    catalog: &ModelCatalog,
    benchmarks: &[BenchmarkResult],
) -> Vec<Recommendation> {
    let mem = MachineMemory::from_profile(profile);
    let cores = profile.cpu.physical_cores.max(profile.cpu.logical_cores).max(1);
    let avx2 = profile.cpu.flags.avx2 || profile.cpu.flags.neon;

    let mut recs: Vec<Recommendation> = catalog
        .models
        .iter()
        .map(|m| rate_one(m, &mem, cores, avx2, benchmarks))
        .collect();

    // Best-first: a composite of quality weighted by how well it runs, so a
    // high-quality model that runs *great* outranks a tiny model that runs
    // *blazing*, while anything that won't run sinks to the bottom.
    recs.sort_by(|a, b| {
        sort_score(b)
            .partial_cmp(&sort_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    recs
}

/// Composite ranking score (higher is better) used only for ordering.
fn sort_score(r: &Recommendation) -> f64 {
    let tier_mult = match r.tier {
        Tier::WontRun => 0.0,
        Tier::Slow => 0.60,
        Tier::Okay => 0.85,
        Tier::Great => 1.00,
        Tier::Blazing => 1.05,
    };
    // Tiny tie-breaker on raw speed so equal-quality models order sensibly.
    let tps = r
        .measured_tokens_per_sec
        .or(r.estimated_tokens_per_sec)
        .unwrap_or(0.0);
    r.quality_score * tier_mult + tps * 0.001
}

// ---------------------------------------------------------------------------
// Memory model
// ---------------------------------------------------------------------------

/// Normalized view of the memory available for inference on this machine.
struct MachineMemory {
    /// VRAM usable for offload (discrete GPUs), or the unified pool on Apple
    /// Silicon. 0 when there is no usable accelerator memory.
    gpu_mb: u64,
    /// System RAM capacity (the fallback / CPU path).
    ram_mb: u64,
    /// True on Apple Silicon unified memory — GPU shares the RAM pool.
    unified: bool,
    /// True when a usable discrete/integrated GPU backend is present.
    has_gpu: bool,
}

impl MachineMemory {
    fn from_profile(p: &HardwareProfile) -> Self {
        let unified = p
            .apple_silicon
            .as_ref()
            .map(|a| a.unified_memory)
            .unwrap_or(false);

        // RAM capacity: prefer total; fall back to available if total is unset.
        let ram_mb = if p.memory.total_mb > 0 {
            p.memory.total_mb
        } else {
            p.memory.available_mb
        };

        if unified {
            // Unified memory: the "GPU" can address (most of) system RAM.
            return MachineMemory {
                gpu_mb: ram_mb,
                ram_mb,
                unified: true,
                has_gpu: true,
            };
        }

        // Discrete path: sum VRAM across non-integrated GPUs that report it.
        let mut gpu_mb = 0u64;
        let mut has_gpu = false;
        for g in &p.gpus {
            if g.is_integrated {
                // iGPU shares system RAM — don't double-count it as VRAM.
                has_gpu = true;
                continue;
            }
            if let Some(v) = g.vram_total_mb {
                if v > 0 {
                    gpu_mb += v;
                    has_gpu = true;
                }
            }
        }

        MachineMemory {
            gpu_mb,
            ram_mb,
            unified: false,
            has_gpu,
        }
    }
}

/// Which memory path a fitted model will actually execute on.
enum ExecPath {
    Gpu,
    Offload,
    Cpu,
}

// ---------------------------------------------------------------------------
// Per-model rating
// ---------------------------------------------------------------------------

fn rate_one(
    m: &CatalogModel,
    mem: &MachineMemory,
    cores: u32,
    avx2: bool,
    benchmarks: &[BenchmarkResult],
) -> Recommendation {
    // Pick the best quant that fits: prefer Q8_0, fall back to Q4_K_M.
    let q8 = find_quant(m, "Q8_0");
    let q4 = find_quant(m, "Q4_K_M");
    // Smallest available quant (for the "won't even run at smallest" check).
    let smallest = m
        .quants
        .iter()
        .min_by_key(|q| q.size_mb)
        .or(q4)
        .unwrap_or(&m.quants[0]);

    // Effective footprint = weights + KV/runtime headroom.
    let footprint = |q: &Quant| ((q.size_mb as f64) * (1.0 + KV_HEADROOM)).ceil() as u64;

    let pool = mem.gpu_mb.max(mem.ram_mb); // best case: whichever is larger
    let chosen: &Quant = if let Some(q) = q8.filter(|q| footprint(q) <= pool) {
        q
    } else if let Some(q) = q4.filter(|q| footprint(q) <= pool) {
        q
    } else {
        // Nothing fits; report against the smallest quant so the memory math is
        // still meaningful (it will be tagged WontRun below).
        smallest
    };

    let required_mb = footprint(chosen);
    let memory_fit = compute_fit(required_mb, mem);
    let fits = memory_fit.fits_gpu || memory_fit.fits_ram;

    // Decide execution path.
    let path = if memory_fit.fits_gpu {
        ExecPath::Gpu
    } else if mem.has_gpu && memory_fit.offload {
        ExecPath::Offload
    } else {
        ExecPath::Cpu
    };

    // Headroom fraction of the pool this model executes against.
    let exec_pool = match path {
        ExecPath::Gpu => mem.gpu_mb,
        ExecPath::Offload => mem.gpu_mb.max(mem.ram_mb),
        ExecPath::Cpu => mem.ram_mb,
    };
    let headroom_frac = if exec_pool > 0 {
        ((exec_pool as f64 - required_mb as f64) / exec_pool as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Look for a measured benchmark for this model.
    let measured = best_benchmark(m, chosen, benchmarks);

    // Estimate (or read) generation speed.
    let (est_tps, measured_tps, ttft_ms, source) = match measured {
        Some(b) => {
            let gen = b.gen_tps.unwrap_or(0.0);
            (None, Some(gen), b.ttft_ms, RatingSource::Measured)
        }
        None => {
            let est = if fits {
                estimate_tps(m.params_b, &path, &memory_fit, headroom_frac, cores, avx2)
            } else {
                // Won't run — no meaningful speed to report.
                0.0
            };
            (Some(est), None, None, RatingSource::Heuristic)
        }
    };

    let speed = measured_tps.or(est_tps).unwrap_or(0.0);
    // A measured run proves the model actually executed, so it always "fits".
    let effectively_fits = fits || source == RatingSource::Measured;
    let tier = classify(effectively_fits, speed);

    // Comfortable context scales with memory headroom.
    let context_comfortable = if !effectively_fits {
        m.context_default.min(2048)
    } else if headroom_frac > COMFORT_HEADROOM {
        m.context_max
    } else if headroom_frac > 0.05 {
        m.context_default
    } else {
        m.context_default.min(4096)
    };

    let why = build_why(
        tier,
        source,
        speed,
        &path,
        &memory_fit,
        context_comfortable,
        &chosen.name,
        mem,
    );

    Recommendation {
        model_id: m.id.clone(),
        display_name: m.display_name.clone(),
        family: m.family.clone(),
        params_b: m.params_b,
        quality_score: m.quality_score,
        quant: chosen.name.clone(),
        tier,
        source,
        estimated_tokens_per_sec: est_tps.map(round1),
        measured_tokens_per_sec: measured_tps.map(round1),
        ttft_ms,
        memory_fit,
        context_comfortable,
        why,
        ollama_pull: chosen
            .ollama_tag
            .clone()
            .unwrap_or_else(|| m.ollama_pull.clone()),
    }
}

fn find_quant<'a>(m: &'a CatalogModel, name: &str) -> Option<&'a Quant> {
    m.quants.iter().find(|q| q.name.eq_ignore_ascii_case(name))
}

fn compute_fit(required_mb: u64, mem: &MachineMemory) -> MemoryFit {
    let fits_gpu = mem.gpu_mb > 0 && required_mb <= mem.gpu_mb;
    let fits_ram = mem.ram_mb > 0 && required_mb <= mem.ram_mb;
    // Offload only makes sense when there's *some* GPU but it's too small alone.
    let offload = mem.has_gpu && !fits_gpu && fits_ram && mem.gpu_mb > 0;
    let gpu_layers_fraction = if fits_gpu {
        1.0
    } else if mem.gpu_mb > 0 && required_mb > 0 {
        (mem.gpu_mb as f64 / required_mb as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    MemoryFit {
        required_mb,
        gpu_available_mb: mem.gpu_mb,
        ram_available_mb: mem.ram_mb,
        fits_gpu,
        fits_ram,
        offload,
        gpu_layers_fraction,
    }
}

/// Heuristic tokens/sec estimate. Conservative by design.
fn estimate_tps(
    params_b: f64,
    path: &ExecPath,
    fit: &MemoryFit,
    headroom_frac: f64,
    cores: u32,
    avx2: bool,
) -> f64 {
    let params_b = params_b.max(0.1);
    match path {
        ExecPath::Gpu => {
            // Anchor: a 7B model at ~75 tok/s, scaling inversely with size.
            let base = 75.0 * (7.0 / params_b).powf(0.7);
            // Tight VRAM → toward the low end of 50-100; roomy → high end.
            let adj = base * (0.70 + 0.50 * headroom_frac);
            adj.clamp(5.0, 120.0)
        }
        ExecPath::Cpu => {
            // Anchor: a 7B model at ~12 tok/s on 8 fast cores with AVX2.
            let base = 12.0 * (7.0 / params_b).powf(0.9);
            let core_factor = (cores as f64 / 8.0).clamp(0.4, 2.0);
            let simd = if avx2 { 1.0 } else { 0.65 };
            (base * core_factor * simd).clamp(0.5, 40.0)
        }
        ExecPath::Offload => {
            // Blend of GPU and CPU speeds, weighted by the fraction of layers on
            // the GPU, then discounted for PCIe transfer overhead.
            let gpu = estimate_tps(params_b, &ExecPath::Gpu, fit, headroom_frac, cores, avx2);
            let cpu = estimate_tps(params_b, &ExecPath::Cpu, fit, headroom_frac, cores, avx2);
            let f = fit.gpu_layers_fraction;
            (cpu + (gpu - cpu) * f * 0.60).clamp(0.5, 100.0)
        }
    }
}

fn classify(fits: bool, tps: f64) -> Tier {
    if !fits {
        return Tier::WontRun;
    }
    if tps < SLOW_MAX_TPS {
        Tier::Slow
    } else if tps < OKAY_MAX_TPS {
        Tier::Okay
    } else if tps < GREAT_MAX_TPS {
        Tier::Great
    } else {
        Tier::Blazing
    }
}

/// Find the most recent successful benchmark that matches this model/quant.
fn best_benchmark<'a>(
    m: &CatalogModel,
    quant: &Quant,
    benchmarks: &'a [BenchmarkResult],
) -> Option<&'a BenchmarkResult> {
    benchmarks
        .iter()
        .filter(|b| b.ok && b.gen_tps.is_some() && benchmark_matches(m, quant, b))
        // Most recent wins; timestamps are ISO-8601 so lexical max == latest.
        .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
}

fn benchmark_matches(m: &CatalogModel, quant: &Quant, b: &BenchmarkResult) -> bool {
    let target = b.model.as_str();
    if target.eq_ignore_ascii_case(&m.id) || target.eq_ignore_ascii_case(&m.ollama_pull) {
        return true;
    }
    if let Some(tag) = &quant.ollama_tag {
        if target.eq_ignore_ascii_case(tag) {
            return true;
        }
    }
    // Match any of the model's other quant tags.
    if m.quants.iter().any(|q| {
        q.ollama_tag
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case(target))
            == Some(true)
    }) {
        return true;
    }
    // Loose: benchmark model shares the ollama repo prefix (before ':').
    let repo = m.ollama_pull.split(':').next().unwrap_or("");
    !repo.is_empty() && target.to_ascii_lowercase().starts_with(&repo.to_ascii_lowercase())
}

#[allow(clippy::too_many_arguments)]
fn build_why(
    tier: Tier,
    source: RatingSource,
    tps: f64,
    path: &ExecPath,
    fit: &MemoryFit,
    context: u32,
    quant: &str,
    mem: &MachineMemory,
) -> String {
    if tier == Tier::WontRun {
        let pool = mem.gpu_mb.max(mem.ram_mb);
        return format!(
            "Won't run: needs ~{} but only {} available even at the smallest quant.",
            fmt_mb(fit.required_mb),
            fmt_mb(pool)
        );
    }

    let where_ = match path {
        ExecPath::Gpu if mem.unified => "unified memory",
        ExecPath::Gpu => "GPU",
        ExecPath::Offload => "GPU + CPU offload",
        ExecPath::Cpu => "CPU",
    };

    let headroom_mb = match path {
        ExecPath::Gpu => mem.gpu_mb.saturating_sub(fit.required_mb),
        ExecPath::Offload => mem.gpu_mb.max(mem.ram_mb).saturating_sub(fit.required_mb),
        ExecPath::Cpu => mem.ram_mb.saturating_sub(fit.required_mb),
    };

    let speed = match source {
        RatingSource::Measured => format!("measured {} tok/s", round1(tps)),
        RatingSource::Heuristic => format!("~{} tok/s", round1(tps)),
    };

    let mut s = format!(
        "{}: {} on {} ({}), {} headroom, {} context comfortable.",
        tier.label(),
        speed,
        where_,
        quant,
        fmt_mb(headroom_mb),
        fmt_ctx(context)
    );
    if let ExecPath::Offload = path {
        s.push_str(&format!(
            " ~{:.0}% of layers on GPU.",
            fit.gpu_layers_fraction * 100.0
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Small formatting helpers
// ---------------------------------------------------------------------------

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn fmt_mb(mb: u64) -> String {
    if mb >= 1024 {
        format!("{:.1}GB", mb as f64 / 1024.0)
    } else {
        format!("{}MB", mb)
    }
}

fn fmt_ctx(ctx: u32) -> String {
    if ctx >= 1024 && ctx % 1024 == 0 {
        format!("{}k", ctx / 1024)
    } else if ctx >= 1000 {
        format!("{}k", ctx / 1000)
    } else {
        ctx.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn base_cpu() -> Cpu {
        Cpu {
            model: "Test CPU".into(),
            vendor: "test".into(),
            physical_cores: 8,
            logical_cores: 16,
            base_clock_mhz: Some(3000.0),
            max_clock_mhz: Some(4500.0),
            flags: CpuFlags {
                avx2: true,
                avx512: false,
                fma: true,
                f16c: true,
                neon: false,
            },
        }
    }

    fn profile(ram_mb: u64, gpus: Vec<Gpu>, apple: Option<AppleSilicon>) -> HardwareProfile {
        HardwareProfile {
            cpu: base_cpu(),
            gpus,
            apple_silicon: apple,
            memory: Memory {
                total_mb: ram_mb,
                available_mb: ram_mb * 3 / 4,
            },
            storage: Storage {
                models_dir: "/tmp".into(),
                free_mb: 500_000,
                read_mbps: Some(3000.0),
            },
            os: Os {
                name: "linux".into(),
                version: "1".into(),
                arch: "x86_64".into(),
            },
            backends: vec!["cpu".into()],
            detected_at: "2026-07-09T00:00:00Z".into(),
        }
    }

    fn discrete_gpu(vram_mb: u64) -> Gpu {
        Gpu {
            vendor: "NVIDIA".into(),
            model: "RTX Test".into(),
            vram_total_mb: Some(vram_mb),
            vram_free_mb: Some(vram_mb),
            driver_version: None,
            cuda_version: Some("12.4".into()),
            compute_capability: Some("8.9".into()),
            backend: "cuda".into(),
            is_integrated: false,
        }
    }

    #[test]
    fn cpu_only_small_machine_rates_and_sorts() {
        let cat = catalog::load_bundled().unwrap();
        let p = profile(16_000, vec![], None); // 16GB CPU-only laptop
        let recs = rate_all(&p, &cat, &[]);
        assert_eq!(recs.len(), cat.models.len());

        // 70B must not run on a 16GB machine.
        let big = recs.iter().find(|r| r.model_id == "llama3.1-70b").unwrap();
        assert_eq!(big.tier, Tier::WontRun);

        // Something small must run.
        assert!(recs.iter().any(|r| r.tier.rank() >= Tier::Okay.rank()));

        // Sorted best-first: score is non-increasing.
        for w in recs.windows(2) {
            assert!(sort_score(&w[0]) >= sort_score(&w[1]) - 1e-9);
        }
        // Everything heuristic (no benchmarks supplied).
        assert!(recs.iter().all(|r| r.source == RatingSource::Heuristic));
    }

    #[test]
    fn big_gpu_runs_large_models_fast() {
        let cat = catalog::load_bundled().unwrap();
        // 48GB VRAM workstation + 64GB RAM.
        let p = profile(64_000, vec![discrete_gpu(48_000)], None);
        let recs = rate_all(&p, &cat, &[]);
        let l8 = recs.iter().find(|r| r.model_id == "llama3.1-8b").unwrap();
        assert!(l8.memory_fit.fits_gpu);
        assert!(matches!(l8.tier, Tier::Great | Tier::Blazing));
        // 8B on a big GPU should pick the higher-quality Q8_0.
        assert_eq!(l8.quant, "Q8_0");
    }

    #[test]
    fn measured_benchmark_overrides_heuristic() {
        let cat = catalog::load_bundled().unwrap();
        let p = profile(32_000, vec![discrete_gpu(24_000)], None);
        let bench = BenchmarkResult {
            model: "llama3.1:8b".into(),
            adapter: "ollama".into(),
            ok: true,
            error: None,
            prompt_eval_tps: Some(900.0),
            gen_tps: Some(7.5), // deliberately slow -> should force Slow tier
            ttft_ms: Some(300.0),
            peak_mem_mb: Some(8000.0),
            context_tested: 512,
            background_load: Some(0.1),
            tier: "quick".into(),
            timestamp: "2026-07-09T04:40:00Z".into(),
        };
        let recs = rate_all(&p, &cat, &[bench]);
        let l8 = recs.iter().find(|r| r.model_id == "llama3.1-8b").unwrap();
        assert_eq!(l8.source, RatingSource::Measured);
        assert_eq!(l8.measured_tokens_per_sec, Some(7.5));
        assert_eq!(l8.tier, Tier::Slow);
    }

    #[test]
    fn apple_unified_uses_ram_pool() {
        let cat = catalog::load_bundled().unwrap();
        let apple = Some(AppleSilicon {
            unified_memory: true,
            gpu_cores: Some(30),
            neural_engine: true,
            chip: "M3 Max".into(),
        });
        let mut p = profile(64_000, vec![], apple);
        p.cpu.flags.neon = true;
        p.backends = vec!["metal".into(), "cpu".into()];
        let recs = rate_all(&p, &cat, &[]);
        // With 64GB unified memory, a 32B model should fit on the "GPU" pool.
        let q32 = recs.iter().find(|r| r.model_id == "qwen2.5-32b").unwrap();
        assert!(q32.memory_fit.fits_gpu);
    }
}
