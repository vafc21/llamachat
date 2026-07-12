//! All drawing for the TUI. Each screen is a small function that reads immutable
//! [`App`] state and paints into the frame; no screen mutates state.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use fitllm_core::{HardwareProfile, Recommendation};

use super::theme::{tier_badge, tier_color, Palette, Theme};
use super::{llama, App, Ollama, Screen, TABS};

pub fn draw(f: &mut Frame, app: &App) {
    let p = app.theme.palette();
    match app.screen {
        Screen::Splash => splash(f, app, &p),
        Screen::ThemePick => theme_pick(f, app, &p),
        Screen::Profiling => profiling(f, app, &p),
        Screen::Ollama => ollama(f, app, &p),
        Screen::Main => main_view(f, app, &p),
    }
}

// --- shared helpers ----------------------------------------------------------

/// A rect centered within `area` at the given width/height (clamped to fit).
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    Rect::new(x, y, w, h)
}

/// The mascot as styled lines in the brand color.
fn llama_lines(tick: u64, p: &Palette) -> Vec<Line<'static>> {
    llama::frame(tick)
        .lines
        .iter()
        .map(|l| Line::from(Span::styled(*l, Style::default().fg(p.brand))).centered())
        .collect()
}

fn fmt_gb(mb: u64) -> String {
    format!("{:.1} GB", mb as f64 / 1024.0)
}

fn fmt_params(b: f64) -> String {
    if b >= 1.0 {
        format!("{b:.0}B")
    } else {
        format!("{:.0}M", b * 1000.0)
    }
}

// --- splash ------------------------------------------------------------------

fn splash(f: &mut Frame, app: &App, p: &Palette) {
    let area = f.area();
    let mut lines = llama_lines(app.tick, p);
    lines.push(Line::from(""));
    lines.push(
        Line::from(Span::styled(
            "LlamaChat",
            Style::default().fg(p.brand).add_modifier(Modifier::BOLD),
        ))
        .centered(),
    );
    lines.push(
        Line::from(Span::styled(
            "Which LLMs will actually run on your machine?",
            Style::default().fg(p.text),
        ))
        .centered(),
    );
    lines.push(Line::from(""));
    lines.push(
        Line::from(Span::styled(
            "measured on your silicon — not spec-sheet guesses",
            Style::default().fg(p.dim).add_modifier(Modifier::ITALIC),
        ))
        .centered(),
    );
    lines.push(Line::from(""));
    lines.push(
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(p.dim)),
            Span::styled("Enter", Style::default().fg(p.accent).add_modifier(Modifier::BOLD)),
            Span::styled(" to begin", Style::default().fg(p.dim)),
        ])
        .centered(),
    );

    let h = lines.len() as u16;
    let rect = centered(area, area.width.min(64), h);
    f.render_widget(Paragraph::new(Text::from(lines)), rect);
    footer(f, area, "Enter continue    ·    q / Esc quit", app.tick, p);
}

// --- theme picker ------------------------------------------------------------

fn theme_pick(f: &mut Frame, app: &App, p: &Palette) {
    let area = f.area();
    let mut lines: Vec<Line> = Vec::new();
    lines.push(
        Line::from(Span::styled("Let's set the mood 🎨", Style::default().fg(p.brand).bold()))
            .centered(),
    );
    lines.push(
        Line::from(Span::styled(
            "Pick a color theme (change anytime)",
            Style::default().fg(p.dim),
        ))
        .centered(),
    );
    lines.push(Line::from(""));

    for (i, t) in Theme::ALL.iter().enumerate() {
        let selected = i == app.theme_cursor;
        let marker = if selected { "❯ " } else { "  " };
        let style = if selected {
            Style::default().fg(p.brand).add_modifier(Modifier::BOLD).bg(p.sel_bg)
        } else {
            Style::default().fg(p.text)
        };
        let mut spans = vec![
            Span::styled(marker, Style::default().fg(p.accent)),
            Span::styled(format!("{:<22}", t.label()), style),
        ];
        if selected {
            spans.push(Span::styled(format!("  {}", t.hint()), Style::default().fg(p.dim)));
        }
        lines.push(Line::from(spans).centered());
    }

    let h = lines.len() as u16;
    let rect = centered(area, area.width.min(72), h);
    f.render_widget(Paragraph::new(Text::from(lines)), rect);
    footer(f, area, "↑/↓ move    ·    Enter select    ·    Esc quit", app.tick, p);
}

// --- profiling (the animated moment) -----------------------------------------

fn profiling(f: &mut Frame, app: &App, p: &Palette) {
    let area = f.area();
    let mut lines = llama_lines(app.tick, p);
    lines.push(Line::from(""));
    lines.push(
        Line::from(vec![
            Span::styled(llama::spinner(app.tick), Style::default().fg(p.accent)),
            Span::raw("  "),
            Span::styled(
                format!("{}…", llama::verb(app.tick)),
                Style::default().fg(p.text).add_modifier(Modifier::BOLD),
            ),
        ])
        .centered(),
    );
    lines.push(Line::from(""));

    // Staged sub-labels, revealed on a timer, purely to show progress.
    let steps = [
        "Reading CPU & instruction sets",
        "Probing GPUs and VRAM",
        "Measuring memory & storage",
        "Rating every model in the catalog",
    ];
    let shown = ((app.tick / 3) as usize).min(steps.len());
    for (i, s) in steps.iter().enumerate() {
        let (glyph, style) = if i < shown {
            ("✓", Style::default().fg(tier_color(3)))
        } else if i == shown {
            (llama::spinner(app.tick), Style::default().fg(p.accent))
        } else {
            ("·", Style::default().fg(p.dim))
        };
        lines.push(
            Line::from(vec![
                Span::styled(format!(" {glyph} "), style),
                Span::styled(*s, Style::default().fg(if i <= shown { p.text } else { p.dim })),
            ])
            .centered(),
        );
    }

    if let Some(err) = &app.load_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(format!("⚠ {err}"), Style::default().fg(tier_color(0)))).centered());
    }

    let h = lines.len() as u16;
    let rect = centered(area, area.width.min(60), h);
    f.render_widget(Paragraph::new(Text::from(lines)), rect);
    footer(f, area, "Profiling your machine…", app.tick, p);
}

// --- ollama nudge ------------------------------------------------------------

fn ollama(f: &mut Frame, app: &App, p: &Palette) {
    let area = f.area();
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled("Ready to run models 🦙", Style::default().fg(p.brand).bold())).centered());
    lines.push(Line::from(""));

    match app.ollama.as_ref() {
        Some(Ollama::Present(v)) => {
            lines.push(
                Line::from(vec![
                    Span::styled("✓ Ollama detected  ", Style::default().fg(tier_color(3)).bold()),
                    Span::styled(v.clone(), Style::default().fg(p.dim)),
                ])
                .centered(),
            );
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "You can pull any recommended model straight from the Models tab.",
                Style::default().fg(p.text),
            )).centered());
        }
        _ => {
            lines.push(Line::from(Span::styled("○ Ollama not found", Style::default().fg(tier_color(1)).bold())).centered());
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "LlamaChat rates models without it, but you'll need Ollama to actually run them:",
                Style::default().fg(p.text),
            )).centered());
            lines.push(Line::from(Span::styled("https://ollama.com/download", Style::default().fg(p.accent).underlined())).centered());
        }
    }
    lines.push(Line::from(""));
    lines.push(
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(p.dim)),
            Span::styled("Enter", Style::default().fg(p.accent).bold()),
            Span::styled(" to see your ratings", Style::default().fg(p.dim)),
        ])
        .centered(),
    );

    let h = lines.len() as u16;
    let rect = centered(area, area.width.min(74), h);
    f.render_widget(Paragraph::new(Text::from(lines)), rect);
    footer(f, area, "Enter continue    ·    Esc quit", app.tick, p);
}

// --- main app ----------------------------------------------------------------

fn main_view(f: &mut Frame, app: &App, p: &Palette) {
    let area = f.area();
    let rows = Layout::vertical([
        Constraint::Length(3), // header + tabs
        Constraint::Min(3),    // body
        Constraint::Length(1), // footer
    ])
    .split(area);

    header(f, rows[0], app, p);
    match app.tab {
        0 => models_tab(f, rows[1], app, p),
        1 => hardware_tab(f, rows[1], app, p),
        _ => about_tab(f, rows[1], app, p),
    }

    let hint = match app.tab {
        0 => "↑/↓ pick model   ·   Tab switch view   ·   q quit",
        _ => "Tab switch view   ·   1/2/3 jump   ·   q quit",
    };
    footer(f, area, hint, app.tick, p);
}

fn header(f: &mut Frame, area: Rect, app: &App, p: &Palette) {
    let cols = Layout::horizontal([Constraint::Length(22), Constraint::Min(0)]).split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {} ", llama::mini(app.tick)), Style::default().fg(p.brand)),
        Span::styled("LlamaChat", Style::default().fg(p.brand).add_modifier(Modifier::BOLD)),
    ]))
    .block(Block::new());
    f.render_widget(title, cols[0]);

    let tabs = Tabs::new(TABS.iter().map(|t| Span::raw(*t)).collect::<Vec<_>>())
        .select(app.tab)
        .style(Style::default().fg(p.dim))
        .highlight_style(Style::default().fg(p.accent).add_modifier(Modifier::BOLD))
        .divider(Span::styled("·", Style::default().fg(p.border)));
    // Sit the tabs on the same row as the title, not below it.
    let tabs_row = Rect::new(cols[1].x, area.y, cols[1].width, 1);
    f.render_widget(tabs, tabs_row);
}

fn models_tab(f: &mut Frame, area: Rect, app: &App, p: &Palette) {
    if app.recs.is_empty() {
        let msg = app
            .load_error
            .clone()
            .unwrap_or_else(|| "No models in the catalog.".into());
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(p.dim)).block(bordered("Models", p)),
            area,
        );
        return;
    }

    let cols = Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)]).split(area);

    // Left: the ranked list.
    let items: Vec<ListItem> = app
        .recs
        .iter()
        .map(|r| {
            let rank = r.tier.rank();
            let color = tier_color(rank);
            let line = Line::from(vec![
                Span::styled(tier_badge(rank), Style::default().fg(color)),
                Span::raw("  "),
                Span::styled(
                    format!("{:<20}", truncate(&r.display_name, 20)),
                    Style::default().fg(p.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:>5}  ", fmt_params(r.params_b)), Style::default().fg(p.dim)),
                Span::styled(format!("🧠{:>3.0}", r.intelligence_score), Style::default().fg(p.accent)),
                Span::styled(format!(" ⚡{:>3.0}  ", r.speed_score), Style::default().fg(p.brand)),
                Span::styled(r.tier.label(), Style::default().fg(color)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(bordered(&format!("Models · {} rated", app.recs.len()), p))
        .highlight_style(Style::default().bg(p.sel_bg).add_modifier(Modifier::BOLD))
        .highlight_symbol("");

    let mut state = ListState::default();
    state.select(Some(app.rec_cursor));
    f.render_stateful_widget(list, cols[0], &mut state);

    // Right: detail on the selected model.
    if let Some(r) = app.selected_rec() {
        f.render_widget(detail_panel(r, p), cols[1]);
    }
}

fn detail_panel(r: &Recommendation, p: &Palette) -> Paragraph<'static> {
    let rank = r.tier.rank();
    let color = tier_color(rank);
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        r.display_name.clone(),
        Style::default().fg(p.brand).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled(format!("{}  ", r.tier.label()), Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(
            match r.source {
                fitllm_core::RatingSource::Measured => "· measured".to_string(),
                fitllm_core::RatingSource::Heuristic => "· estimated".to_string(),
            },
            Style::default().fg(p.dim),
        ),
    ]));
    lines.push(Line::from(""));

    let kv = |k: &str, v: String, p: &Palette| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{k:<13} "), Style::default().fg(p.dim)),
            Span::styled(v, Style::default().fg(p.text)),
        ])
    };
    lines.push(kv("Intelligence", format!("{:.1} / 10", r.intelligence_score), p));
    lines.push(kv("Speed", format!("{:.1} / 10", r.speed_score), p));
    lines.push(kv("Size", format!("{} params · {}", fmt_params(r.params_b), r.quant), p));
    let tps = r
        .measured_tokens_per_sec
        .or(r.estimated_tokens_per_sec)
        .map(|t| format!("{t:.0} tok/s"))
        .unwrap_or_else(|| "—".into());
    lines.push(kv("Throughput", tps, p));
    lines.push(kv("Context", format!("{} tokens", r.context_comfortable), p));

    let mf = &r.memory_fit;
    let fit = if mf.fits_gpu {
        "fits in VRAM".to_string()
    } else if mf.fits_ram {
        format!("in RAM ({:.0}% on GPU)", mf.gpu_layers_fraction * 100.0)
    } else {
        "too big to fit".to_string()
    };
    lines.push(kv("Memory", format!("{} · {}", fmt_gb(mf.required_mb), fit), p));

    lines.push(Line::from(""));
    for wl in wrap_words(&r.why, 40) {
        lines.push(Line::from(Span::styled(wl, Style::default().fg(p.dim).add_modifier(Modifier::ITALIC))));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Pull & run:", Style::default().fg(p.dim))));
    lines.push(Line::from(Span::styled(
        format!("  ollama run {}", r.ollama_pull),
        Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
    )));

    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: true })
        .block(bordered("Details", p))
}

fn hardware_tab(f: &mut Frame, area: Rect, app: &App, p: &Palette) {
    let Some(hw) = app.profile.as_ref() else {
        f.render_widget(Paragraph::new("Hardware not detected yet.").block(bordered("Hardware", p)), area);
        return;
    };
    let lines = hardware_lines(hw, app, p);
    f.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }).block(bordered("Hardware", p)),
        area,
    );
}

fn hardware_lines(hw: &HardwareProfile, app: &App, p: &Palette) -> Vec<Line<'static>> {
    let head = |s: &str, p: &Palette| Line::from(Span::styled(s.to_string(), Style::default().fg(p.brand).add_modifier(Modifier::BOLD)));
    let row = |k: &str, v: String, p: &Palette| {
        Line::from(vec![
            Span::styled(format!("  {k:<14}"), Style::default().fg(p.dim)),
            Span::styled(v, Style::default().fg(p.text)),
        ])
    };
    let mut lines: Vec<Line> = Vec::new();

    lines.push(head("CPU", p));
    lines.push(row("Model", hw.cpu.model.clone(), p));
    lines.push(row("Cores", format!("{} physical · {} logical", hw.cpu.physical_cores, hw.cpu.logical_cores), p));
    let mut flags = Vec::new();
    if hw.cpu.flags.avx2 { flags.push("AVX2"); }
    if hw.cpu.flags.avx512 { flags.push("AVX-512"); }
    if hw.cpu.flags.neon { flags.push("NEON"); }
    if hw.cpu.flags.fma { flags.push("FMA"); }
    lines.push(row("Accel", if flags.is_empty() { "—".into() } else { flags.join(", ") }, p));

    lines.push(Line::from(""));
    lines.push(head("GPU", p));
    if hw.gpus.is_empty() {
        lines.push(row("", "none detected".into(), p));
    } else {
        for g in &hw.gpus {
            let vram = g.vram_total_mb.map(fmt_gb).unwrap_or_else(|| "shared".into());
            lines.push(row(
                &g.vendor,
                format!("{}  ·  {}  ·  {}{}", g.model, vram, g.backend, if g.is_integrated { " (integrated)" } else { "" }),
                p,
            ));
        }
    }

    lines.push(Line::from(""));
    lines.push(head("Memory & storage", p));
    lines.push(row("RAM", format!("{} total · {} free", fmt_gb(hw.memory.total_mb), fmt_gb(hw.memory.available_mb)), p));
    lines.push(row("Disk", format!("{} free", fmt_gb(hw.storage.free_mb)), p));

    lines.push(Line::from(""));
    lines.push(head("System", p));
    lines.push(row("OS", format!("{} {} · {}", hw.os.name, hw.os.version, hw.os.arch), p));
    lines.push(row("Backends", if hw.backends.is_empty() { "cpu".into() } else { hw.backends.join(", ") }, p));
    let ollama = match app.ollama.as_ref() {
        Some(Ollama::Present(v)) => format!("✓ {v}"),
        _ => "not installed".into(),
    };
    lines.push(row("Ollama", ollama, p));

    lines
}

fn about_tab(f: &mut Frame, area: Rect, app: &App, p: &Palette) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled("LlamaChat", Style::default().fg(p.brand).add_modifier(Modifier::BOLD))));
    lines.push(Line::from(Span::styled(
        format!("v{} · local-first · zero telemetry", env!("CARGO_PKG_VERSION")),
        Style::default().fg(p.dim),
    )));
    lines.push(Line::from(""));
    for l in [
        "Profiles your hardware, benchmarks open models on your own",
        "silicon, and rates them from “Won't run” to “Blazing”.",
        "",
        "This terminal UI is a front door to the same Rust engine the",
        "desktop app uses — every rating you see is real.",
    ] {
        lines.push(Line::from(Span::styled(l, Style::default().fg(p.text))));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Keys", Style::default().fg(p.brand).add_modifier(Modifier::BOLD))));
    for (k, v) in [
        ("↑ / ↓", "move selection"),
        ("Tab / 1·2·3", "switch views"),
        ("← / →", "prev / next view"),
        ("q / Esc", "quit"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:<14}"), Style::default().fg(p.accent)),
            Span::styled(v, Style::default().fg(p.text)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("Catalog: {} models rated on this machine", app.catalog_count),
        Style::default().fg(p.dim),
    )));

    f.render_widget(Paragraph::new(Text::from(lines)).block(bordered("About", p)), area);
}

// --- chrome ------------------------------------------------------------------

fn footer(f: &mut Frame, area: Rect, hint: &str, tick: u64, p: &Palette) {
    let row = Rect::new(area.x, area.y + area.height.saturating_sub(1), area.width, 1);
    let line = Line::from(vec![
        Span::styled(format!(" {} ", llama::mini(tick)), Style::default().fg(p.brand)),
        Span::styled(hint.to_string(), Style::default().fg(p.dim)),
    ]);
    f.render_widget(Paragraph::new(line), row);
}

fn bordered(title: &str, p: &Palette) -> Block<'static> {
    Block::bordered()
        .title(Span::styled(format!(" {title} "), Style::default().fg(p.brand).add_modifier(Modifier::BOLD)))
        .border_style(Style::default().fg(p.border))
}

// --- tiny text utilities -----------------------------------------------------

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Greedy word wrap to `width` columns (ASCII-ish; good enough for the `why`
/// blurb, which the panel also soft-wraps).
fn wrap_words(s: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.len() + 1 + word.len() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
