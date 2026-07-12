//! All drawing for the TUI. Each screen is a small function that reads immutable
//! [`App`] state and paints into the frame; no screen mutates state.

use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use llamachat_core::{HardwareProfile, Recommendation};

use super::chat::{Chat, Role};
use super::theme::{tier_badge, tier_color, Palette, Theme};
use super::{brand, llama, App, Ollama, Overlay, Screen, TABS};

pub fn draw(f: &mut Frame, app: &App) {
    let p = app.theme.palette();
    match app.screen {
        Screen::Splash => splash(f, app, &p),
        Screen::ThemePick => theme_pick(f, app, &p),
        Screen::Profiling => profiling(f, app, &p),
        Screen::Ollama => ollama(f, app, &p),
        Screen::Main => main_view(f, app, &p),
        Screen::Chat => chat_view(f, app, &p),
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

/// The brand llama logo as centered lines, with a soft shimmer that sweeps down
/// the rows and then pauses — subtle "it's alive" motion, no jitter. Rows are
/// equal width so centering keeps the block aligned.
fn logo_lines(art: &[&'static str], p: &Palette, tick: u64) -> Vec<Line<'static>> {
    let n = art.len() as i64;
    let cycle = n + 8; // sweep the highlight down, then rest for a few ticks
    let pos = ((tick / 2) as i64) % cycle;
    art.iter()
        .enumerate()
        .map(|(i, l)| {
            let d = (i as i64 - pos).abs();
            let style = if d == 0 {
                Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
            } else if d == 1 {
                Style::default().fg(p.brand).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(p.brand)
            };
            Line::from(Span::styled(*l, style)).centered()
        })
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
    let mut lines = logo_lines(&brand::LOGO, p, app.tick);
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
        Line::from(Span::styled("Let's set the mood", Style::default().fg(p.brand).bold()))
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
    let mut lines = logo_lines(&brand::LOGO, p, app.tick);
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
    lines.push(Line::from(Span::styled("Ready to run models", Style::default().fg(p.brand).bold())).centered());
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
        0 => "↑/↓ pick   ·   Enter download   ·   r run   ·   Tab views   ·   q quit",
        _ => "Tab switch view   ·   1/2/3 jump   ·   q quit",
    };
    footer(f, area, hint, app.tick, p);

    // Pull progress / result modal sits on top of everything.
    if !matches!(app.overlay, Overlay::None) {
        overlay(f, area, app, p);
    }
}

fn header(f: &mut Frame, area: Rect, app: &App, p: &Palette) {
    let cols = Layout::horizontal([Constraint::Length(22), Constraint::Min(0)]).split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("  LlamaChat", Style::default().fg(p.brand).add_modifier(Modifier::BOLD)),
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
            let installed = app.installed.contains(&r.ollama_pull);
            let line = Line::from(vec![
                Span::styled(
                    if installed { "● " } else { "  " },
                    Style::default().fg(p.accent),
                ),
                Span::styled(tier_badge(rank), Style::default().fg(color)),
                Span::raw("  "),
                Span::styled(
                    format!("{:<20}", truncate(&r.display_name, 20)),
                    Style::default().fg(p.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:>5}  ", fmt_params(r.params_b)), Style::default().fg(p.dim)),
                Span::styled(format!("iq{:>3.0}", r.intelligence_score), Style::default().fg(p.accent)),
                Span::styled(format!(" sp{:>3.0}  ", r.speed_score), Style::default().fg(p.brand)),
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
                llamachat_core::RatingSource::Measured => "· measured".to_string(),
                llamachat_core::RatingSource::Heuristic => "· estimated".to_string(),
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

fn footer(f: &mut Frame, area: Rect, hint: &str, _tick: u64, p: &Palette) {
    let row = Rect::new(area.x, area.y + area.height.saturating_sub(1), area.width, 1);
    let line = Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(hint.to_string(), Style::default().fg(p.dim)),
    ]);
    f.render_widget(Paragraph::new(line), row);
}

/// The download-progress / result modal, drawn over the Models tab.
fn overlay(f: &mut Frame, area: Rect, app: &App, p: &Palette) {
    let w = area.width.saturating_sub(8).clamp(32, 76);
    let h = 14u16.min(area.height.saturating_sub(2)).max(8);
    let rect = centered(area, w, h);
    let inner_w = (w as usize).saturating_sub(4);
    f.render_widget(Clear, rect);

    let (title, mut lines, hint): (String, Vec<Line>, String) = match &app.overlay {
        Overlay::Pulling(job) => {
            let mut ls: Vec<Line> = Vec::new();
            ls.push(
                Line::from(vec![
                    Span::styled(llama::spinner(app.tick), Style::default().fg(p.accent)),
                    Span::raw(" "),
                    Span::styled(
                        format!("{}…", llama::verb(app.tick)),
                        Style::default().fg(p.text).add_modifier(Modifier::BOLD),
                    ),
                ]),
            );
            ls.push(Line::from(""));
            let log = job.lines();
            let rows = (h as usize).saturating_sub(6);
            for l in log.iter().rev().take(rows).rev() {
                ls.push(Line::from(Span::styled(
                    truncate(l, inner_w),
                    Style::default().fg(p.dim),
                )));
            }
            (format!("Downloading {}", job.display), ls, "Ctrl-C to abort".into())
        }
        Overlay::Result { ok, display, msg, .. } => {
            let mut ls: Vec<Line> = Vec::new();
            let (glyph, col) = if *ok { ("✓", tier_color(3)) } else { ("✗", tier_color(0)) };
            ls.push(Line::from(Span::styled(
                format!("{glyph} {display}"),
                Style::default().fg(col).add_modifier(Modifier::BOLD),
            )));
            ls.push(Line::from(""));
            for wl in wrap_words(msg, inner_w) {
                ls.push(Line::from(Span::styled(wl, Style::default().fg(p.text))));
            }
            let hint = if *ok {
                "Enter / r  run it    ·    Esc  back".to_string()
            } else {
                "Esc  back".to_string()
            };
            let title = if *ok { "Ready" } else { "Couldn't download" };
            (title.to_string(), ls, hint)
        }
        Overlay::None => return,
    };

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(hint, Style::default().fg(p.accent))));

    let block = Block::bordered()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(p.brand).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(p.accent));
    f.render_widget(
        Paragraph::new(Text::from(lines)).block(block).wrap(Wrap { trim: true }),
        rect,
    );
}

// --- chat screen -------------------------------------------------------------

fn chat_view(f: &mut Frame, app: &App, p: &Palette) {
    let area = f.area();
    let Some(c) = app.chat.as_ref() else { return };

    // Keep the logo "up" as a persistent banner when there's vertical room;
    // fall back to a one-line header on short terminals so chat isn't squeezed.
    let banner = area.height >= 24;
    let header_h = if banner { brand::LOGO_SM.len() as u16 + 1 } else { 1 };

    let rows = Layout::vertical([
        Constraint::Length(header_h),
        Constraint::Min(1),    // transcript
        Constraint::Length(3), // input box
    ])
    .split(area);

    if banner {
        chat_banner(f, rows[0], app, c, p);
    } else {
        chat_header_compact(f, rows[0], app, c, p);
    }

    let body = rows[1];
    if c.messages.is_empty() && !c.is_streaming() {
        chat_tips(f, body, c, p);
    } else {
        let inner = body.inner(Margin::new(1, 0));
        let width = inner.width as usize;
        let lines = chat_transcript(c, width, p, app.tick);
        let total = lines.len();
        let h = inner.height as usize;
        let base = total.saturating_sub(h);
        let off = base.saturating_sub(c.scroll as usize) as u16;
        f.render_widget(Paragraph::new(Text::from(lines)).scroll((off, 0)), inner);
    }

    chat_input(f, rows[2], c, p);
    if c.slash_query().is_some() {
        slash_palette(f, rows[2], c, p);
    }
}

/// The persistent, animated logo banner that stays at the top of the chat.
fn chat_banner(f: &mut Frame, area: Rect, app: &App, c: &Chat, p: &Palette) {
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    // Animated logo (rows are pre-centered).
    f.render_widget(Paragraph::new(Text::from(logo_lines(&brand::LOGO_SM, p, app.tick))), rows[0]);
    // Status line: model name + either streaming stats or the key hints.
    let mut spans = vec![Span::styled(
        c.model.clone(),
        Style::default().fg(p.brand).add_modifier(Modifier::BOLD),
    )];
    if c.is_streaming() {
        spans.push(Span::styled(format!("  ·  {} ", llama::spinner(app.tick)), Style::default().fg(p.accent)));
        spans.push(Span::styled(
            format!("{}… {}t {}s", llama::verb(app.tick), c.stream_tokens(), c.stream_elapsed()),
            Style::default().fg(p.dim),
        ));
    } else {
        spans.push(Span::styled(
            "   ·   / commands   ·   ↑↓ scroll   ·   Esc back",
            Style::default().fg(p.dim),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans).centered()), rows[1]);
}

/// One-line chat header for short terminals.
fn chat_header_compact(f: &mut Frame, area: Rect, app: &App, c: &Chat, p: &Palette) {
    let cols = Layout::horizontal([Constraint::Min(10), Constraint::Length(40)]).split(area);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {}", c.model),
            Style::default().fg(p.brand).add_modifier(Modifier::BOLD),
        ))),
        cols[0],
    );
    let right = if c.is_streaming() {
        Line::from(vec![
            Span::styled(llama::spinner(app.tick), Style::default().fg(p.accent)),
            Span::styled(
                format!(" {}… {}t {}s ", llama::verb(app.tick), c.stream_tokens(), c.stream_elapsed()),
                Style::default().fg(p.dim),
            ),
        ])
    } else {
        Line::from(Span::styled("/ commands · ↑↓ scroll · Esc back ", Style::default().fg(p.dim)))
    };
    f.render_widget(Paragraph::new(right).alignment(ratatui::layout::Alignment::Right), cols[1]);
}

/// Empty-state tips shown under the banner before the first message.
fn chat_tips(f: &mut Frame, area: Rect, c: &Chat, p: &Palette) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(
        Line::from(vec![
            Span::styled("Chatting with ", Style::default().fg(p.text)),
            Span::styled(c.model.clone(), Style::default().fg(p.brand).add_modifier(Modifier::BOLD)),
        ])
        .centered(),
    );
    lines.push(Line::from(""));
    for tip in [
        "Ask it anything — replies stream in live.",
        "Type  /  for commands · Enter to send · Esc to go back.",
    ] {
        lines.push(Line::from(Span::styled(tip, Style::default().fg(p.dim))).centered());
    }
    let h = lines.len() as u16;
    let rect = centered(area, area.width.min(60), h);
    f.render_widget(Paragraph::new(Text::from(lines)), rect);
}

fn chat_input(f: &mut Frame, area: Rect, c: &Chat, p: &Palette) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.accent));
    let inner_w = area.width.saturating_sub(4) as usize;

    let line = if c.input.is_empty() {
        Line::from(vec![
            Span::styled("› ", Style::default().fg(p.accent)),
            Span::styled(
                format!("Message {}…   ( / for commands )", c.model),
                Style::default().fg(p.dim),
            ),
        ])
    } else {
        // Show the tail of the input so the caret stays visible.
        let shown: String = tail(&c.input, inner_w.saturating_sub(3));
        Line::from(vec![
            Span::styled("› ", Style::default().fg(p.accent)),
            Span::styled(shown, Style::default().fg(p.text)),
            Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)),
        ])
    };
    f.render_widget(Paragraph::new(line).block(block), area);
}

fn slash_palette(f: &mut Frame, input_area: Rect, c: &Chat, p: &Palette) {
    let matches = c.slash_matches();
    if matches.is_empty() {
        return;
    }
    let h = (matches.len() as u16 + 2).min(input_area.y.saturating_sub(2)).max(3);
    let w = 52.min(input_area.width);
    let rect = Rect::new(input_area.x, input_area.y.saturating_sub(h), w, h);
    f.render_widget(Clear, rect);

    let sel = c.slash_selected.min(matches.len().saturating_sub(1));
    let items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let selected = i == sel;
            let name = Style::default()
                .fg(if selected { p.brand } else { p.accent })
                .add_modifier(Modifier::BOLD);
            ListItem::new(Line::from(vec![
                Span::styled(if selected { "❯ " } else { "  " }, Style::default().fg(p.brand)),
                Span::styled(format!("/{:<8}", cmd.name), name),
                Span::styled(format!("  {}", cmd.desc), Style::default().fg(p.dim)),
            ]))
        })
        .collect();
    let list = List::new(items).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(p.border))
            .title(Span::styled(" commands ", Style::default().fg(p.dim))),
    );
    f.render_widget(list, rect);
}

/// Build the whole transcript as styled lines (newest last).
fn chat_transcript(c: &Chat, width: usize, p: &Palette, tick: u64) -> Vec<Line<'static>> {
    let width = width.max(8);
    let mut out: Vec<Line> = Vec::new();
    for m in &c.messages {
        match m.role {
            Role::User => {
                for (i, wl) in wrap_words(&m.content, width.saturating_sub(2)).into_iter().enumerate() {
                    let pref = if i == 0 { "› " } else { "  " };
                    out.push(Line::from(vec![
                        Span::styled(pref, Style::default().fg(p.accent).add_modifier(Modifier::BOLD)),
                        Span::styled(wl, Style::default().fg(p.text).add_modifier(Modifier::BOLD)),
                    ]));
                }
            }
            Role::Assistant => {
                out.push(Line::from(vec![
                    Span::styled("▌ ", Style::default().fg(p.brand)),
                    Span::styled("llama", Style::default().fg(p.brand).add_modifier(Modifier::BOLD)),
                ]));
                for l in render_md(&m.content, width, Style::default().fg(p.text), p) {
                    out.push(l);
                }
            }
            Role::System => {
                for l in render_md(&m.content, width, Style::default().fg(p.dim), p) {
                    out.push(l);
                }
            }
        }
        out.push(Line::from(""));
    }
    if c.is_streaming() {
        out.push(Line::from(vec![
            Span::styled(llama::spinner(tick), Style::default().fg(p.accent)),
            Span::styled(
                format!("  {}… (esc to interrupt)", llama::verb(tick)),
                Style::default().fg(p.dim),
            ),
        ]));
    }
    out
}

/// Minimal markdown → styled lines: code fences, `inline code`, **bold**,
/// headings, and bullet lists, hard-wrapped to `width` so scroll math is exact.
fn render_md(text: &str, width: usize, base: Style, p: &Palette) -> Vec<Line<'static>> {
    let width = width.max(8);
    let mut out: Vec<Line> = Vec::new();
    let mut in_code = false;
    for raw in text.split('\n') {
        if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            for chunk in hard_wrap(raw, width.saturating_sub(2)) {
                out.push(Line::from(vec![
                    Span::styled("▏ ", Style::default().fg(p.border)),
                    Span::styled(chunk, Style::default().fg(p.accent)),
                ]));
            }
            continue;
        }
        let t = raw.trim_end();
        if let Some(h) = t
            .strip_prefix("### ")
            .or_else(|| t.strip_prefix("## "))
            .or_else(|| t.strip_prefix("# "))
        {
            for l in wrap_words(h, width) {
                out.push(Line::from(Span::styled(
                    l,
                    Style::default().fg(p.brand).add_modifier(Modifier::BOLD),
                )));
            }
            continue;
        }
        let (marker, body) = match t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            Some(b) => ("• ", b),
            None => ("", t),
        };
        if body.is_empty() {
            out.push(Line::from(""));
            continue;
        }
        let tokens = inline_tokens(body, base, p);
        let wrapped = wrap_spans(tokens, width.saturating_sub(marker.chars().count()));
        for (i, mut spans) in wrapped.into_iter().enumerate() {
            if !marker.is_empty() {
                let pref = if i == 0 { marker } else { "  " };
                spans.insert(0, Span::styled(pref, Style::default().fg(p.accent)));
            }
            out.push(Line::from(spans));
        }
    }
    out
}

/// Split text into styled runs, toggling on `**bold**` and `` `code` ``.
fn inline_tokens(text: &str, base: Style, p: &Palette) -> Vec<(String, Style)> {
    let mut tokens: Vec<(String, Style)> = Vec::new();
    let mut buf = String::new();
    let mut bold = false;
    let mut code = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let flush = |buf: &mut String, tokens: &mut Vec<(String, Style)>, bold: bool, code: bool| {
        if buf.is_empty() {
            return;
        }
        let mut style = base;
        if code {
            style = Style::default().fg(p.accent);
        }
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        tokens.push((std::mem::take(buf), style));
    };
    while i < chars.len() {
        if !code && chars[i] == '*' && chars.get(i + 1) == Some(&'*') {
            flush(&mut buf, &mut tokens, bold, code);
            bold = !bold;
            i += 2;
            continue;
        }
        if chars[i] == '`' {
            flush(&mut buf, &mut tokens, bold, code);
            code = !code;
            i += 1;
            continue;
        }
        buf.push(chars[i]);
        i += 1;
    }
    flush(&mut buf, &mut tokens, bold, code);
    tokens
}

/// Word-wrap a sequence of styled tokens to `width`, preserving styles.
fn wrap_spans(tokens: Vec<(String, Style)>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(4);
    let mut lines: Vec<Vec<Span>> = vec![vec![]];
    let mut w = 0usize;
    for (text, style) in tokens {
        for word in text.split(' ') {
            if word.is_empty() {
                continue;
            }
            // Hard-split words longer than the whole line.
            for piece in hard_wrap(word, width) {
                let need = piece.chars().count();
                let sep = if w > 0 { 1 } else { 0 };
                if w > 0 && w + sep + need > width {
                    lines.push(vec![]);
                    w = 0;
                }
                if w > 0 {
                    lines.last_mut().unwrap().push(Span::raw(" "));
                    w += 1;
                }
                lines.last_mut().unwrap().push(Span::styled(piece, style));
                w += need;
            }
        }
    }
    lines
}

/// Hard-wrap a string to `width` columns by character count.
fn hard_wrap(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= width {
        return vec![s.to_string()];
    }
    chars.chunks(width).map(|c| c.iter().collect()).collect()
}

/// The last `max` characters of a string (keeps the input caret in view).
fn tail(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        s.chars().skip(n - max).collect()
    }
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
