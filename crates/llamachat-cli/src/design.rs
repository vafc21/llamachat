//! `llamachat design` — static design previews for the CLI redesign.
//!
//! This is a **review surface**, not the real TUI. It prints each candidate
//! direction to stdout with the styling it would actually use, so the look can
//! be judged (and screenshotted) without rebuilding 3,000 lines of `tui/` three
//! times over. Once a direction is picked, `tui/render.rs` gets rewritten to
//! match and this module goes away.
//!
//! Design law shared by every variant:
//! - **One accent** — `#4d7cff`, the same accent as the desktop app. No rainbow.
//! - **No boxes.** Whitespace, alignment and dim text carry the structure.
//! - **No emoji.** Line glyphs only.
//! - Colour is never the only signal; everything reads in `NO_COLOR`.

use std::io::IsTerminal;

// ── styling ────────────────────────────────────────────────────────────────

/// Whether to emit colour at all. Honours `NO_COLOR` and non-tty stdout, the
/// way Codex and Claude Code both do.
fn colour_on() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

struct Style {
    on: bool,
}

impl Style {
    fn new() -> Self {
        Style { on: colour_on() }
    }
    fn wrap(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    /// The single accent — brand blue, matching the desktop app.
    fn accent(&self, s: &str) -> String {
        self.wrap("38;2;77;124;255", s)
    }
    /// Secondary text. Dim rather than a second hue.
    fn dim(&self, s: &str) -> String {
        self.wrap("38;2;108;108;124", s)
    }
    /// Closes with `22` (bold off) rather than `0` (reset everything). A full
    /// reset inside a background band would drop the band's fill for the rest
    /// of the line.
    fn bold(&self, s: &str) -> String {
        if self.on { format!("\x1b[1m{s}\x1b[22m") } else { s.to_string() }
    }
    fn ok(&self, s: &str) -> String {
        self.wrap("38;2;63;185;80", s)
    }
    fn warn(&self, s: &str) -> String {
        self.wrap("38;2;210;153;34", s)
    }
    /// Section label: dim, letterspaced, uppercase. Replaces every box border.
    fn label(&self, s: &str) -> String {
        let spaced: String = s
            .to_uppercase()
            .chars()
            .flat_map(|c| [c, ' '])
            .collect::<String>()
            .trim_end()
            .to_string();
        self.dim(&spaced)
    }
}

/// A model row, kept static on purpose — this is a design preview.
struct Row {
    name: &'static str,
    tag: &'static str,
    params: &'static str,
    size: &'static str,
    speed: &'static str,
    /// 0 won't run … 4 blazing
    rank: u8,
}

const ROWS: &[Row] = &[
    Row { name: "Qwen3 30B A3B",      tag: "qwen3:30b-a3b",   params: "30.5B", size: "17.3G", speed: "62 tok/s",  rank: 3 },
    Row { name: "Gemma 3 12B",        tag: "gemma3:12b",      params: "12B",   size: "8.1G",  speed: "94 tok/s",  rank: 4 },
    Row { name: "Llama 3.2 3B",       tag: "llama3.2:3b",     params: "3B",    size: "2.0G",  speed: "180 tok/s", rank: 4 },
    Row { name: "DeepSeek R1 8B",     tag: "deepseek-r1:8b",  params: "8B",    size: "4.9G",  speed: "88 tok/s",  rank: 3 },
    Row { name: "Phi-3 Medium 14B",   tag: "phi3:14b",        params: "14B",   size: "7.9G",  speed: "41 tok/s",  rank: 2 },
    Row { name: "Qwen 2.5 32B",       tag: "qwen2.5:32b",     params: "32B",   size: "19.8G", speed: "12 tok/s",  rank: 1 },
    Row { name: "Llama 3.3 70B",      tag: "llama3.3:70b",    params: "71B",   size: "42.5G", speed: "—",         rank: 0 },
];

fn rank_word(rank: u8) -> &'static str {
    match rank {
        0 => "won't run",
        1 => "slow",
        2 => "okay",
        3 => "great",
        _ => "blazing",
    }
}

/// A 5-cell meter in ONE hue. The old TUI used five different colours for this;
/// the fill level alone carries the meaning, so colour doesn't have to.
fn meter(s: &Style, rank: u8) -> String {
    let filled = rank as usize + 1;
    let bar: String = (0..5)
        .map(|i| if i < filled { '▰' } else { '▱' })
        .collect();
    if rank == 0 {
        s.dim(&bar)
    } else {
        s.accent(&bar)
    }
}

// ── variant A: inline ──────────────────────────────────────────────────────

/// **Inline.** No alternate screen at all — the session is printed into normal
/// scrollback and stays there after exit, so you can pipe it, search it, and
/// scroll it with the terminal's own scrollbar. Only the composer is a live
/// region pinned to the bottom. This is the Claude Code / Codex model, and it
/// is the single biggest departure from what `llamachat` does today.
fn variant_a(s: &Style) {
    println!();
    println!("  {}  {}", s.bold("llamachat"), s.dim("0.1.0   qwen3:30b-a3b · 48/48 layers on GPU"));
    println!();
    println!("  {} {}", s.accent("›"), "which of my models is best for long documents?");
    println!();
    println!("  Gemma 3 12B. It has the largest usable context on this machine");
    println!("  once the KV cache is quantized, and it still holds 94 tok/s.");
    println!();
    println!("  {}  {}", s.dim("·"), s.dim("read  hardware profile                            0.2s"));
    println!("  {}  {}", s.dim("·"), s.dim("rated 13 models against 64G / 24G VRAM            1.1s"));
    println!();
    println!("  {}", s.dim("gemma3:12b · 94 tok/s · 1,204 tokens · ctx 3,180/32,768"));
    println!();
    println!("  {} {}", s.accent("›"), s.dim("Ask anything"));
    println!("     {}", s.dim("/ commands   ⏎ send   ⌃C quit"));
    println!();
}

// ── variant B: flat ────────────────────────────────────────────────────────

/// **Flat.** Keeps a full-screen app, but every box border is deleted. Section
/// identity comes from a dim letterspaced label and column alignment instead.
/// Same information density as today with roughly half the ink.
fn variant_b(s: &Style) {
    println!();
    println!("  {}   {}", s.bold("llamachat"), s.dim("models · hardware · about"));
    println!();
    println!("  {}", s.label("models"));
    println!();
    for r in ROWS {
        let sel = r.name == "Gemma 3 12B";
        let marker = if sel { s.accent("›") } else { " ".into() };
        // Pad the PLAIN text first, then style it. Styling first would let the
        // escape codes count toward the field width and knock the column out.
        let name_cell = format!("{:<22}", r.name);
        let name = if sel { s.bold(&name_cell) } else { name_cell };
        println!(
            "  {} {} {}  {}  {}   {}  {}",
            marker,
            name,
            s.dim(&format!("{:>6}", r.params)),
            s.dim(&format!("{:>6}", r.size)),
            s.dim(&format!("{:>9}", r.speed)),
            meter(s, r.rank),
            if r.rank == 0 { s.dim(rank_word(r.rank)) } else { rank_word(r.rank).to_string() },
        );
    }
    println!();
    println!("  {}", s.label("gemma 3 12b"));
    println!();
    let kv = |k: &str, v: &str| println!("  {}{}", s.dim(&format!("{:<12}", k)), v);
    kv("runs", "blazing · 94 tok/s, fully on GPU");
    kv("memory", "8.1 GB of 24 GB VRAM · 15.9 GB headroom");
    kv("context", "32,768 tokens · KV f16 2.1 GB");
    kv("pull", &s.accent("ollama run gemma3:12b"));
    println!();
    println!("  {}", s.dim("↑↓ pick   ⏎ download   r run   tab views   q quit"));
    println!();
}

// ── variant C: workbench ───────────────────────────────────────────────────

/// **Workbench.** Full-screen and two-column like today, but the boxes become a
/// single vertical rule, and a persistent status line carries the runtime the
/// way the desktop app's status bar does.
fn variant_c(s: &Style) {
    let rule = s.dim("│");
    println!();
    println!("  {}   {}", s.bold("llamachat"), s.dim("models · hardware · about"));
    println!();
    let right: [(&str, String); 7] = [
        ("runs", format!("{} · 94 tok/s", s.ok("blazing"))),
        ("params", "12B · Q4_K_M".into()),
        ("size", "8.1 GB".into()),
        ("vram", "8.1 / 24 GB".into()),
        ("context", "32,768".into()),
        ("kv cache", "f16 · 2.1 GB".into()),
        ("pull", s.accent("ollama run gemma3:12b")),
    ];
    // Every left-hand cell is a fixed width of PLAIN text, so the rule lands in
    // the same column on every row no matter how a cell is styled.
    for (i, r) in ROWS.iter().enumerate() {
        let sel = r.name == "Gemma 3 12B";
        let marker = if sel { s.accent("›") } else { " ".into() };
        let name_cell = format!("{:<20}", r.name);
        let name = if sel { s.bold(&name_cell) } else { name_cell };
        let tag_cell = format!("{:<18}", r.tag);
        let left = format!("  {} {} {}  {}", marker, name, meter(s, r.rank), s.dim(&tag_cell));
        match right.get(i) {
            Some((k, v)) => println!("{left}  {rule}  {}{}", s.dim(&format!("{:<10}", k)), v),
            None => println!("{left}  {rule}"),
        }
    }
    println!();
    println!(
        "  {}  {}  {}  {}",
        s.dim("gemma3:12b"),
        s.dim("cpu 18%"),
        s.dim("gpu 87%  vram 20.3/24G"),
        s.dim("ctx 3,180/32,768"),
    );
    println!();
}

// ── shared: chat + approval, rendered in the picked direction ───────────────

/// The chat surface and the approval prompt — the two screens where the current
/// TUI is weakest (a truncating popup and a bordered composer).
fn chat_surface(s: &Style) {
    println!();
    println!("  {}", s.label("chat"));
    println!();
    println!("  {} {}", s.accent("›"), "rename every screenshot in ~/Pictures by date");
    println!();
    println!("  I'll read the folder first, then rename in one pass.");
    println!();
    println!("  {}  {}", s.ok("✓"), s.dim("list   ~/Pictures                        142 files   0.1s"));
    println!("  {}  {}", s.ok("✓"), s.dim("read   exif dates                        142 files   0.9s"));
    println!("  {}  {}", s.accent("▸"), s.dim("rename 142 files                              running"));
    println!();
    // Bounded output, the way Codex caps tool output: head, marker, tail.
    println!("  {}", s.dim("  IMG_0041.png → 2026-07-14-153002.png"));
    println!("  {}", s.dim("  IMG_0042.png → 2026-07-14-153311.png"));
    println!("  {}", s.dim("  … 138 more lines, full output in the transcript"));
    println!("  {}", s.dim("  IMG_0183.png → 2026-07-31-090114.png"));
    println!();
    println!("  {}", s.label("approval"));
    println!();
    println!("  {} wants to {} 142 files in {}", s.bold("gemma3:12b"), s.warn("rename"), s.bold("~/Pictures"));
    println!();
    println!("    {}  allow once", s.accent("a"));
    println!("    {}  allow for the rest of this session", s.accent("A"));
    println!("    {}  deny", s.accent("d"));
    println!();
    println!("  {}", s.dim("esc denies · nothing runs until you choose"));
    println!();
}

// ── variant D: designed ────────────────────────────────────────────────────
//
// A, B and C only ever coloured the *foreground*. That is why they read as
// plain text: no depth, no fills, no hierarchy beyond dim-vs-bright. D uses the
// things a terminal actually gives you — background fills, eighth-block
// geometry, shading, sparklines, filled badges — while staying in one hue.
// Every colour below is a tint or shade of the brand blue #4d7cff, plus
// neutrals. That keeps "one accent" true while giving the thing weight.

const W: usize = 84;

/// Tints and shades of the single accent, plus the neutral surfaces. Named the
/// way the desktop app's tokens are so the two products stay legible together.
mod tone {
    pub const BG: (u8, u8, u8) = (13, 13, 18); // #0d0d12  app background
    pub const SURFACE: (u8, u8, u8) = (22, 22, 31); // #16161f  cards / bands
    pub const RAISED: (u8, u8, u8) = (30, 31, 43); // selected row
    pub const BAR: (u8, u8, u8) = (18, 18, 26); // header / status bar
    pub const TEXT: (u8, u8, u8) = (227, 227, 232);
    pub const MUTED: (u8, u8, u8) = (140, 140, 157);
    pub const FAINT: (u8, u8, u8) = (90, 90, 107);
    pub const ACCENT: (u8, u8, u8) = (77, 124, 255); // #4d7cff
    pub const ACCENT_DIM: (u8, u8, u8) = (48, 74, 150);
    pub const ACCENT_DEEP: (u8, u8, u8) = (28, 42, 86);
    pub const OK: (u8, u8, u8) = (63, 185, 80);
    pub const WARN: (u8, u8, u8) = (210, 153, 34);
}

impl Style {
    fn fg(&self, c: (u8, u8, u8), s: &str) -> String {
        if self.on { format!("\x1b[38;2;{};{};{}m{s}\x1b[39m", c.0, c.1, c.2) } else { s.into() }
    }
    /// A full-width band. `plain_len` is the *visible* length of `styled`, which
    /// the caller knows and the escape codes would otherwise corrupt.
    fn band(&self, bg: (u8, u8, u8), styled: &str, plain_len: usize) -> String {
        let pad = W.saturating_sub(plain_len);
        if !self.on {
            return format!("{styled}{:pad$}", "", pad = pad);
        }
        format!(
            "\x1b[48;2;{};{};{}m{styled}{:pad$}\x1b[0m",
            bg.0, bg.1, bg.2, "", pad = pad
        )
    }
    /// A filled badge — background block, not an outline. `back_to` is the
    /// background of the row it sits in, restored afterwards; resetting to the
    /// terminal default here would punch a hole in the band.
    fn badge(&self, bg: (u8, u8, u8), fg: (u8, u8, u8), text: &str, back_to: (u8, u8, u8)) -> String {
        if !self.on {
            return format!("[{text}]");
        }
        format!(
            "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m {text} \x1b[48;2;{};{};{}m\x1b[39m",
            bg.0, bg.1, bg.2, fg.0, fg.1, fg.2, back_to.0, back_to.1, back_to.2
        )
    }
}

/// An eighth-block meter: solid accent for the filled part, a shaded trough for
/// the rest. Reads as a machined gauge instead of five loose glyphs.
fn gauge(s: &Style, frac: f32, cells: usize) -> String {
    const EIGHTHS: [&str; 9] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
    let total = (frac.clamp(0.0, 1.0) * cells as f32 * 8.0).round() as usize;
    let full = total / 8;
    let rem = total % 8;
    let mut bar = "█".repeat(full.min(cells));
    if full < cells && rem > 0 {
        bar.push_str(EIGHTHS[rem]);
    }
    let drawn = full.min(cells) + usize::from(full < cells && rem > 0);
    let trough = "░".repeat(cells.saturating_sub(drawn));
    format!("{}{}", s.fg(tone::ACCENT, &bar), s.fg(tone::ACCENT_DEEP, &trough))
}

/// A sparkline from the eighth-block ramp — for live CPU/GPU history.
fn spark(s: &Style, vals: &[f32]) -> String {
    const RAMP: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let line: String = vals
        .iter()
        .map(|v| RAMP[((v.clamp(0.0, 1.0) * 7.0).round()) as usize])
        .collect();
    s.fg(tone::ACCENT_DIM, &line)
}

fn variant_d(s: &Style) {
    // ── header bar ─────────────────────────────────────────────
    let left = format!("  LlamaChat");
    let right = "qwen3:30b-a3b · 62.4 tok/s  ";
    let gap = W.saturating_sub(left.chars().count() + right.chars().count());
    let styled = format!(
        "{}{}{:gap$}{}",
        s.fg(tone::ACCENT, "  "),
        s.bold(&s.fg(tone::TEXT, "LlamaChat")),
        "",
        s.fg(tone::MUTED, right),
        gap = gap
    );
    println!("{}", s.band(tone::BAR, &styled, left.chars().count() + gap + right.chars().count()));
    println!();

    // ── section label with an accent tick ──────────────────────
    let sec = |t: &str| {
        println!(
            "  {} {}",
            s.fg(tone::ACCENT, "▎"),
            s.bold(&s.fg(tone::TEXT, t))
        );
        println!();
    };

    sec("Models");
    for r in ROWS.iter().take(5) {
        let sel = r.name == "Gemma 3 12B";
        let frac = (r.rank as f32 + 1.0) / 5.0;
        let tier = match r.rank {
            4 => Some(("BEST", tone::ACCENT, tone::BG)),
            3 => Some(("GOOD", tone::ACCENT_DEEP, tone::ACCENT)),
            _ => None,
        };
        let row_bg = if sel { tone::RAISED } else { tone::BG };
        let badge = match tier {
            Some((t, bg, fg)) => s.badge(bg, fg, t, row_bg),
            None => "      ".to_string(),
        };
        let name_p = format!("{:<18}", r.name);
        let tag_p = format!("{:<16}", r.tag);
        let size_p = format!("{:>6}", r.size);
        let speed_p = format!("{:>9}", r.speed);
        // 4 lead + 6 badge + 2 + 18 + 16 + 6 + 2 + 12 gauge + 2 + 9
        let plain_len = 4 + 6 + 2 + 18 + 16 + 6 + 2 + 12 + 2 + 9;
        let styled = format!(
            "{}{}  {}{}{}  {}  {}",
            if sel { s.fg(tone::ACCENT, "  ▎ ") } else { "    ".into() },
            badge,
            if sel { s.bold(&s.fg(tone::TEXT, &name_p)) } else { s.fg(tone::TEXT, &name_p) },
            s.fg(tone::FAINT, &tag_p),
            s.fg(tone::MUTED, &size_p),
            gauge(s, frac, 12),
            s.fg(tone::MUTED, &speed_p),
        );
        println!("{}", s.band(row_bg, &styled, plain_len));
    }
    println!();

    // ── detail card, as a filled surface ───────────────────────
    sec("Gemma 3 12B");
    // Measure the value instead of hand-counting it — the hand-counted lengths
    // were off, which made each band a slightly different width.
    let card = |k: &str, v: &str, accent: bool| {
        let styled = format!(
            "    {}{}",
            s.fg(tone::FAINT, &format!("{:<14}", k)),
            s.fg(if accent { tone::ACCENT } else { tone::TEXT }, v)
        );
        println!("{}", s.band(tone::SURFACE, &styled, 4 + 14 + v.chars().count()));
    };
    println!("{}", s.band(tone::SURFACE, "", 0));
    card("runs", "blazing — 94 tok/s, entirely on the GPU", false);
    card("memory", "8.1 GB of 24 GB VRAM · 15.9 GB headroom", false);
    card("context", "32,768 tokens · KV f16 2.1 GB", false);
    card("pull", "ollama run gemma3:12b", true);
    println!("{}", s.band(tone::SURFACE, "", 0));
    println!();

    // ── hardware, with sparklines ──────────────────────────────
    sec("This machine");
    let hw = |name: &str, frac: f32, hist: &[f32], detail: &str| {
        let name_p = format!("{:<8}", name);
        let det_p = format!("{:<22}", detail);
        let pct_p = format!("{:>5}", format!("{}%", (frac * 100.0) as i32));
        let plain = 4 + 8 + 16 + 2 + 12 + 2 + 22 + 5;
        let styled = format!(
            "    {}{}  {}  {}{}",
            s.fg(tone::TEXT, &name_p),
            spark(s, hist),
            gauge(s, frac, 12),
            s.fg(tone::MUTED, &det_p),
            s.fg(tone::TEXT, &pct_p),
        );
        println!("{}", s.band(tone::BG, &styled, plain));
    };
    hw("GPU", 0.87, &[0.2, 0.3, 0.5, 0.4, 0.6, 0.8, 0.9, 0.85, 0.87, 0.9, 0.88, 0.87, 0.9, 0.86, 0.87, 0.87], "RTX 4090 · 24 GB");
    hw("VRAM", 0.85, &[0.5, 0.5, 0.6, 0.6, 0.7, 0.8, 0.85, 0.85, 0.85, 0.85, 0.85, 0.85, 0.85, 0.85, 0.85, 0.85], "20.3 of 24 GB");
    hw("CPU", 0.18, &[0.4, 0.3, 0.2, 0.25, 0.2, 0.15, 0.2, 0.18, 0.16, 0.2, 0.18, 0.17, 0.18, 0.2, 0.18, 0.18], "16 cores · 32 threads");
    hw("RAM", 0.33, &[0.3, 0.3, 0.31, 0.32, 0.33, 0.33, 0.33, 0.34, 0.33, 0.33, 0.33, 0.33, 0.33, 0.33, 0.33, 0.33], "21.4 of 64 GB");
    println!();

    // ── the character, working ─────────────────────────────────
    sec("Working");
    println!(
        "    {}  {}   {}",
        s.fg(tone::ACCENT, "◐"),
        s.fg(tone::TEXT, "Reading hardware profile"),
        s.fg(tone::FAINT, "4s · 892 tokens")
    );
    println!("    {}  {}", s.fg(tone::FAINT, " "), gauge(s, 0.62, 24));
    println!();

    // ── status bar ─────────────────────────────────────────────
    let segs = [
        ("qwen3:30b-a3b", tone::TEXT),
        ("Q4_K_M", tone::MUTED),
        ("62.4 tok/s", tone::MUTED),
        ("ctx 20,317/32,768", tone::MUTED),
        ("ask before changes", tone::WARN),
    ];
    let mut plain = 2usize;
    let mut styled = String::from("  ");
    for (i, (t, c)) in segs.iter().enumerate() {
        if i > 0 {
            styled.push_str(&s.fg(tone::ACCENT_DEEP, " │ "));
            plain += 3;
        }
        styled.push_str(&s.fg(*c, t));
        plain += t.chars().count();
    }
    println!("{}", s.band(tone::BAR, &styled, plain));
    println!();
}

// ── how should the rating actually read? ───────────────────────────────────

/// The `▰▰▰▰▱` glyph is the weakest thing in the row. It renders inconsistently
/// across fonts, and it encodes a five-step scale that a single word already
/// says better. Same five models, six ways of showing "will this run here".
fn rating_treatments(s: &Style) {
    let names: Vec<String> = ROWS.iter().take(5).map(|r| format!("{:<18}", r.name)).collect();

    let show = |title: &str, note: &str, render: &dyn Fn(&Row) -> String| {
        println!("  {} {}", s.fg(tone::ACCENT, "▎"), s.bold(&s.fg(tone::TEXT, title)));
        println!("    {}", s.fg(tone::FAINT, note));
        println!();
        for (i, r) in ROWS.iter().take(5).enumerate() {
            println!("      {}{}", s.fg(tone::TEXT, &names[i]), render(r));
        }
        println!();
    };

    show(
        "1 · Blocks",
        "what you have now — five loose glyphs, font-dependent",
        &|r| meter(s, r.rank),
    );
    show(
        "2 · Gauge",
        "eighth blocks against a shaded trough; reads as one object",
        &|r| gauge(s, (r.rank as f32 + 1.0) / 5.0, 12),
    );
    show(
        "3 · Word only",
        "no bar at all — the word already is the rating",
        &|r| {
            let w = rank_word(r.rank);
            if r.rank == 0 { s.fg(tone::FAINT, w) } else { s.fg(tone::TEXT, w) }
        },
    );
    show(
        "4 · Word + speed",
        "the rating and the number that justifies it, together",
        &|r| {
            let w = format!("{:<9}", rank_word(r.rank));
            format!("{}{}", s.fg(tone::TEXT, &w), s.fg(tone::MUTED, &format!("{:>9}", r.speed)))
        },
    );
    show(
        "5 · Gauge + word",
        "bar for scanning, word for certainty",
        &|r| {
            format!(
                "{}  {}",
                gauge(s, (r.rank as f32 + 1.0) / 5.0, 10),
                if r.rank == 0 { s.fg(tone::FAINT, rank_word(r.rank)) } else { s.fg(tone::TEXT, rank_word(r.rank)) }
            )
        },
    );
    show(
        "6 · Nothing",
        "sort best-first and let the order carry it; speed is the only number",
        &|r| s.fg(tone::MUTED, &format!("{:>9}", r.speed)),
    );

    println!("  {}", s.fg(tone::FAINT, "My pick is 4 — it survives NO_COLOR, any font, and any width,"));
    println!("  {}", s.fg(tone::FAINT, "and it never asks you to decode a glyph. 5 if you want the scan."));
    println!();
}

// ── one row grammar, everywhere ────────────────────────────────────────────

/// Vlad's question: the model rows in B look right — is that language used all
/// around? Today it isn't; models get the meter and everything else gets a
/// different shape. This is the same row applied to five different lists:
/// `marker · name · meter · detail · state`.
fn rows_everywhere(s: &Style) {
    let row = |marker: &str, name: &str, meter_cell: &str, detail: &str, state: &str| {
        println!(
            "  {} {} {}  {}  {}",
            marker,
            s.bold(&format!("{:<20}", name)),
            meter_cell,
            s.dim(&format!("{:<26}", detail)),
            state
        );
    };

    println!();
    println!("  {}", s.label("models"));
    println!();
    for r in ROWS.iter().take(3) {
        row(" ", r.name, &gauge(s, (r.rank as f32 + 1.0) / 5.0, 10), r.tag, &if r.rank == 0 {
            s.dim(rank_word(r.rank))
        } else {
            rank_word(r.rank).to_string()
        });
    }

    println!();
    println!("  {}", s.label("downloads"));
    println!();
    row(" ", "Gemma 3 12B", &gauge(s, 1.0, 10), "8.1G of 8.1G", &s.ok("ready"));
    row(" ", "DeepSeek R1 8B", &gauge(s, 0.63, 10), "3.1G of 4.9G", &s.accent("63%"));
    row(" ", "Qwen 2.5 32B", &gauge(s, 0.0, 10), "queued", &s.dim("waiting"));

    println!();
    println!("  {}", s.label("tools"));
    println!();
    // For tools the meter is the *permission* level — off / ask / on — not a
    // speed rating. Same glyphs, and the column still means "how much of this
    // is turned on", which is what keeps the grammar honest across lists.
    row(" ", "filesystem", &gauge(s, 1.0, 10), "read · write · list", &s.ok("on"));
    row(" ", "shell", &gauge(s, 0.6, 10), "run commands", &s.warn("ask"));
    row(" ", "computer", &gauge(s, 0.0, 10), "click · type · screenshot", &s.dim("off"));

    println!();
    println!("  {}", s.label("hardware"));
    println!();
    row(" ", "GPU", &gauge(s, 0.87, 10), "RTX 4090 · 24 GB", "87%");
    row(" ", "CPU", &gauge(s, 0.18, 10), "16 cores · 32 threads", "18%");
    row(" ", "RAM", &gauge(s, 0.33, 10), "64 GB", "21.4 GB");

    println!();
    println!("  {}", s.label("tasks"));
    println!();
    row(" ", "Rename screenshots", &gauge(s, 0.7, 10), "142 files", &s.accent("running"));
    row(" ", "Sort receipts", &gauge(s, 1.0, 10), "62 files", &s.ok("done"));
    row(" ", "Back up vault", &gauge(s, 0.0, 10), "needs confirmation", &s.warn("blocked"));
    println!();
    println!("  {}", s.dim("Same five columns every time: marker · name · meter · detail · state."));
    println!();
}

// ── entry point ────────────────────────────────────────────────────────────

pub fn run(variant: Option<&str>) {
    let s = Style::new();
    let pick = variant.unwrap_or("all").to_lowercase();

    let head = |title: &str, blurb: &str| {
        println!();
        println!("{}", s.dim(&"─".repeat(78)));
        println!("  {}   {}", s.bold(title), s.dim(blurb));
        println!("{}", s.dim(&"─".repeat(78)));
    };

    if pick == "a" || pick == "all" {
        head("A · Inline", "no alternate screen — session stays in scrollback");
        variant_a(&s);
    }
    if pick == "b" || pick == "all" {
        head("B · Flat", "full-screen, every border deleted");
        variant_b(&s);
    }
    if pick == "c" || pick == "all" {
        head("C · Workbench", "two columns, one rule, persistent status line");
        variant_c(&s);
    }
    if pick == "d" || pick == "all" {
        head("D · Designed", "background fills, gauges, sparklines — one hue, with depth");
        variant_d(&s);
    }
    if pick == "meters" || pick == "all" {
        head("Rating", "six ways to show \"will this run here\"");
        rating_treatments(&s);
    }
    if pick == "rows" || pick == "all" {
        head("One row grammar", "B's model row, applied to every list");
        rows_everywhere(&s);
    }
    if pick == "all" {
        head("Shared", "chat + approval, the two weakest screens today");
        chat_surface(&s);
    }

    if pick == "all" {
        println!();
        println!("  {}", s.dim("llamachat design --variant a|b|c   to see one on its own"));
        println!();
    }
}
