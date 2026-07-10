# Design: Leveled, hardware-sized benchmark phase

Status: proposed (redesign) · Supersedes the "intensity tiers" behavior in SPEC §2.2 / §4.2
Owner: FitLLM · Date: 2026-07-10

## Why this redesign

Reported symptom: on a Mac with an **M4** (a very capable machine), running the
**Full** level recommended a **3B** model. That is backwards — Full on strong
hardware should reach for a large, high-quality model, not a tiny one.

Root cause — two independent flaws:

1. **Levels controlled benchmark *duration*, not *which model runs*.**
   `Quick / Balanced / Full` were passed straight through to the sidecar as a
   measurement-depth knob (`benchmark(model, tier)`). The *set of models* was the
   same regardless of level, so no level ever "reached higher" for a bigger model.
   The onboarding copy reinforced this — it described only time ("~30 seconds",
   "a few minutes", "thorough"), never capability.

2. **Candidate models were "whatever is installed, first 4" — never sized to the
   machine's ceiling.** `run_benchmark` built candidates from
   `catalog models whose tag is installed locally … .take(4)`. So it deeply
   benchmarked whatever small model happened to be installed and surfaced it. It
   never considered larger models the M4 can clearly run, and it never used the
   `HardwareProfile` (unified memory, chip, cores) that the recommender already has.

Net effect: a single, opaque, global outcome that ignored the hardware and never
told the user, per level, **which model it was about to run**.

## The new model

**A level is not a benchmark duration. It is how far up your hardware you want to
push — and every level names the model(s) it will run, before it runs them, and
lets you change them.**

Three levels, each computed against the live `HardwareProfile` + catalog:

| Level        | Meaning                                              | Picks (highest quality among…)      |
|--------------|------------------------------------------------------|-------------------------------------|
| **Quick**    | Start now. The fastest strong fit.                   | models rated **Blazing**            |
| **Standard** | The everyday best. Big as it gets while staying snappy. | models rated **Great or Blazing** |
| **Max**¹     | Push the machine. The best model it can run at all.  | models rated **Okay or better** (anything that runs) |

¹ "Max" replaces "Full". An optional **All** mode benchmarks the *entire* runnable
set rather than just the single best pick, for users who want the full spread.

On an M4, `Max` resolves to a large model (e.g. a 14B/32B-class fit), never a 3B.
On a small laptop, `Max` might legitimately *be* a 3B — because that's the ceiling
there. The number is honest to the machine, and it is shown either way.

### Hard rules

1. **The level selects a model *set* sized to the hardware** via the recommender's
   existing fit tiers (`WontRun → Slow → Okay → Great → Blazing`) — **not** "first
   N installed".
2. **Before running, each level shows the exact model(s) it will run**, with the
   1–10 **intelligence** and **speed** scores and the memory headroom. Per-stage
   disclosure — never one opaque global choice up front.
3. **A level may include not-yet-installed models.** If the planned model isn't
   local, offer to pull it (the per-row Download button already exists). We do not
   silently downgrade to whatever happens to be installed.
4. **The user can override / swap the model at any level** before starting.
5. **Measurement depth becomes a separate, secondary toggle** (short vs thorough
   probe), decoupled from *which* model runs. Depth changes accuracy; level changes
   ambition.

## Selection algorithm

```
plan_levels(profile, catalog, benchmarks) -> LevelPlan:
    rated = rate_all(profile, catalog, benchmarks)   # already exists
    quick    = best_quality(rated where tier == Blazing)      or fastest_that_runs(rated)
    standard = best_quality(rated where tier >= Great)        or quick
    max      = best_quality(rated where tier >= Okay)         or standard
    all      = [r in rated where tier >= Okay]                # optional full-spread mode
    return LevelPlan { quick, standard, max, all }   # each carries model + scores + headroom + why
```

`best_quality` ranks by the catalog `quality_score` (the "how smart" source),
tie-broken by estimated tok/s. Because it's filtered by fit tier first, a level
can only ever name a model that actually runs at that comfort level on this box.

## Surfaces to change (implementation plan)

- **`crates/fitllm-core/src/recommend.rs`** — add `plan_levels(...) -> LevelPlan`
  and a `LevelPlan` type in `types.rs` (each entry: model id, display name,
  intelligence 1–10, speed 1–10, fit tier, headroom, `why`). Pure function, unit-
  testable with fixture profiles (add an M4-like fixture whose `Max` is not a 3B).
- **`src-tauri/src/commands.rs` `run_benchmark`** — replace the "first 4 installed"
  candidate logic with the set from `plan_levels` for the chosen level; take
  measurement **depth** as a separate argument. Add a `benchmark_plan` command that
  returns the `LevelPlan` so the UI can render it *before* running.
- **UI (`ui/src`)** — each level card shows "Runs: `<model>` · smart X/10 · fast
  Y/10 · `<headroom>`" from `benchmark_plan`, with a **Change** control and a
  Download affordance when the model isn't local. Reframe `INTENSITY_OPTIONS` copy
  from time-first to capability-first (time shown as a secondary line).
- **`SPEC.md` §2.2 / §4.2** — updated to describe levels as capability targets that
  name their model, with depth as an independent knob.

## Success check

- On an M4-class profile, `Max` (and usually `Standard`) resolves to a large model,
  never a 3B — asserted by a unit test on `plan_levels`.
- Every level renders its concrete model + scores *before* the run starts.
- Changing the level changes the named model; changing depth does not.
