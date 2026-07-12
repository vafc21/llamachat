//! The LlamaChat mascot: an animated ASCII llama plus the "spinner verbs" that
//! rotate underneath it while the app is thinking.
//!
//! This is the little animal that "does stuff" — it blinks, chews its cud, and
//! swishes its tail on a frame timer, the same way Claude Code's spinner cycles
//! through gerunds. The art is intentionally fixed-width (every rendered line is
//! the same visual width) so the frames don't jitter horizontally as the eyes
//! and mouth change. All frame strings are `'static` constants selected per
//! tick — nothing is allocated while animating.

/// One animation frame of the mascot: 7 equal-width lines.
pub struct LlamaFrame {
    pub lines: [&'static str; 7],
}

/// The mascot, drawn frame-by-frame. `tick` is a monotonically increasing
/// animation counter (one step per render poll); the function derives a blink /
/// chew / tail cycle from it so the llama feels alive without any per-frame
/// bookkeeping in the app state. Every line is 13 columns wide.
pub fn frame(tick: u64) -> LlamaFrame {
    // Small, mutually-prime cycles so the motions never line up mechanically.
    let blink = tick % 14 == 0;
    let chew = (tick / 2) % 2 == 0;
    let tail_up = (tick / 3) % 2 == 0;

    // Every string below is exactly 13 columns wide (see the fixed-width test).
    let line0 = if tail_up { "   .\"\"\".   ~ " } else { "   .\"\"\".   , " };
    let line1 = if blink { "  ( - - )    " } else { "  ( o o )    " };
    let line4 = if chew { "   /www\\     " } else { "   /vvv\\     " };

    LlamaFrame {
        lines: [
            line0,
            line1,
            "   \\   /     ",
            "    | |      ",
            line4,
            "  /|   |\\    ",
            "   |___|     ",
        ],
    }
}

/// A compact one-line llama for headers / footers.
pub fn mini(tick: u64) -> &'static str {
    if tick % 12 == 0 {
        "(-.-)~"
    } else {
        "(o.o)~"
    }
}

/// Braille throbber frames — the classic spinner that spins next to the verb.
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner(tick: u64) -> &'static str {
    SPINNER[(tick as usize) % SPINNER.len()]
}

/// LlamaChat's answer to Claude Code's spinner verbs — llama-flavored gerunds
/// (llamas genuinely ruminate and pronk) mixed with honest what-it's-doing
/// words. Rotated slowly so the user can actually read them.
pub const VERBS: [&str; 24] = [
    "Ruminating",
    "Grazing the silicon",
    "Chewing the cud",
    "Sizing up your GPU",
    "Weighing the weights",
    "Counting FLOPs",
    "Measuring memory",
    "Herding the models",
    "Surveying the pasture",
    "Pronking",
    "Trotting the catalog",
    "Munching quants",
    "Spitting out numbers",
    "Profiling",
    "Percolating",
    "Noodling",
    "Cogitating",
    "Wrangling VRAM",
    "Sniffing out backends",
    "Tallying cores",
    "Warming up",
    "Crunching",
    "Ambling along",
    "Almost there",
];

pub fn verb(tick: u64) -> &'static str {
    // One verb every ~9 ticks so it reads, not flickers.
    VERBS[((tick / 9) as usize) % VERBS.len()]
}
