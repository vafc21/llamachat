//! Color themes for the TUI. Mirrors Claude Code's first onboarding question
//! ("pick a theme"): the user chooses Dark / Light / Auto with the arrow keys
//! and everything downstream reads its colors from the resulting [`Palette`].

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
    Auto,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Dark, Theme::Light, Theme::Auto];

    pub fn label(&self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::Auto => "Auto (match terminal)",
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            Theme::Dark => "Warm llama tones on a dark terminal",
            Theme::Light => "Softer tones tuned for light terminals",
            Theme::Auto => "Use your terminal's own background",
        }
    }

    pub fn palette(&self) -> Palette {
        match self {
            // Auto behaves like Dark but never paints a background, so it sits on
            // whatever the terminal already uses.
            Theme::Light => Palette::light(),
            _ => Palette::dark(),
        }
    }
}

/// The resolved color set the renderer reads from.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Llama tan — the brand accent used for the mascot and titles.
    pub brand: Color,
    /// Secondary accent for highlights / selected rows.
    pub accent: Color,
    pub text: Color,
    pub dim: Color,
    pub border: Color,
    /// Background for the selected row (foreground stays `text`/`brand`).
    pub sel_bg: Color,
}

impl Palette {
    pub fn dark() -> Self {
        Palette {
            brand: Color::Rgb(232, 176, 102), // warm llama tan
            accent: Color::Rgb(120, 200, 220), // soft cyan
            text: Color::Rgb(230, 230, 230),
            dim: Color::Rgb(140, 140, 150),
            border: Color::Rgb(90, 90, 100),
            sel_bg: Color::Rgb(52, 48, 40),
        }
    }

    pub fn light() -> Self {
        Palette {
            brand: Color::Rgb(160, 100, 20),
            accent: Color::Rgb(20, 110, 140),
            text: Color::Rgb(30, 30, 35),
            dim: Color::Rgb(110, 110, 120),
            border: Color::Rgb(180, 180, 190),
            sel_bg: Color::Rgb(235, 224, 205),
        }
    }
}

/// The five run tiers, each with a color that reads at a glance: red → green →
/// blazing cyan. Kept here so the recommendations list and the detail panel
/// agree on what "Blazing" looks like.
pub fn tier_color(rank: u8) -> Color {
    match rank {
        0 => Color::Rgb(150, 90, 90),   // Won't run — muted red
        1 => Color::Rgb(210, 120, 70),  // Slow — orange
        2 => Color::Rgb(215, 195, 90),  // Okay — yellow
        3 => Color::Rgb(120, 200, 110), // Great — green
        _ => Color::Rgb(110, 220, 230), // Blazing — bright cyan
    }
}

/// A short glyph badge for a tier, so the list scans vertically.
pub fn tier_badge(rank: u8) -> &'static str {
    match rank {
        0 => "▄▁▁▁▁",
        1 => "▄▄▁▁▁",
        2 => "▄▄▄▁▁",
        3 => "▄▄▄▄▁",
        _ => "▄▄▄▄▄",
    }
}
