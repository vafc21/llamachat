//! The LlamaChat interactive terminal UI.
//!
//! A Claude-Code-style TUI front door to the `fitllm-core` engine: an animated
//! llama mascot, an arrow-key onboarding wizard, and a live view of the real
//! hardware profile + ranked model recommendations for *this* machine. Nothing
//! here is mocked — every number comes from `hardware::profile()`,
//! `catalog::load_bundled()`, and `recommend::rate_all()`.
//!
//! Layout of this module:
//! - [`App`] holds all UI state and the input/tick reducers.
//! - [`llama`] is the mascot (art + spinner verbs).
//! - [`theme`] is the color palette.
//! - [`render`] draws each screen.

pub mod llama;
pub mod render;
pub mod theme;

use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use fitllm_core::{catalog, hardware, recommend, HardwareProfile, Recommendation};

use theme::Theme;

/// How long one animation tick is. Also the input poll timeout, so the mascot
/// keeps moving even while the user isn't typing.
const TICK: Duration = Duration::from_millis(110);
/// Minimum time the profiling animation stays up, so the llama is actually seen
/// doing its thing even when detection finishes in a few milliseconds.
const PROFILE_MIN: Duration = Duration::from_millis(1500);

/// Which screen of the wizard / app we're on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Splash,
    ThemePick,
    Profiling,
    Ollama,
    Main,
}

/// Whether Ollama (needed to actually run models) is installed.
#[derive(Debug, Clone)]
pub enum Ollama {
    Present(String),
    Absent,
}

/// Result of the background detection job.
struct Loaded {
    profile: HardwareProfile,
    recs: Vec<Recommendation>,
    catalog_count: usize,
    ollama: Ollama,
}

/// Main tabs on the [`Screen::Main`] view.
pub const TABS: [&str; 3] = ["Models", "Hardware", "About"];

/// All UI state.
pub struct App {
    pub screen: Screen,
    pub theme: Theme,
    pub theme_cursor: usize,
    pub tab: usize,
    pub rec_cursor: usize,
    pub tick: u64,
    pub should_quit: bool,

    // Populated once the background job lands.
    pub profile: Option<HardwareProfile>,
    pub recs: Vec<Recommendation>,
    pub catalog_count: usize,
    pub ollama: Option<Ollama>,

    profile_started: Option<Instant>,
    job: Option<Receiver<Result<Loaded, String>>>,
    pub load_error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        App {
            screen: Screen::Splash,
            theme: Theme::Dark,
            theme_cursor: 0,
            tab: 0,
            rec_cursor: 0,
            tick: 0,
            should_quit: false,
            profile: None,
            recs: Vec::new(),
            catalog_count: 0,
            ollama: None,
            profile_started: None,
            job: None,
            load_error: None,
        }
    }
}

impl App {
    /// Kick off hardware detection + rating on a background thread so the UI
    /// keeps animating while the engine works.
    fn start_profiling(&mut self) {
        self.screen = Screen::Profiling;
        self.profile_started = Some(Instant::now());
        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(load_everything());
        });
    }

    /// One animation step: advance the clock, poll the background job, and run
    /// any time-based screen transitions.
    fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);

        // Drain the background job if it has finished.
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok(Ok(loaded)) => {
                    self.profile = Some(loaded.profile);
                    self.recs = loaded.recs;
                    self.catalog_count = loaded.catalog_count;
                    self.ollama = Some(loaded.ollama);
                    self.job = None;
                }
                Ok(Err(e)) => {
                    self.load_error = Some(e);
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.load_error = Some("hardware detection thread stopped unexpectedly".into());
                    self.job = None;
                }
            }
        }

        // Leave the profiling screen once detection is done *and* the minimum
        // animation time has elapsed.
        if self.screen == Screen::Profiling {
            let done = self.job.is_none();
            let waited = self
                .profile_started
                .map(|t| t.elapsed() >= PROFILE_MIN)
                .unwrap_or(true);
            if done && waited {
                self.screen = Screen::Ollama;
            }
        }
    }

    /// Reduce a key press into a state change.
    fn on_key(&mut self, key: KeyEvent) {
        // Global quits.
        if matches!(key.code, KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL)) {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('q') if self.screen == Screen::Main || self.screen == Screen::Splash => {
                self.should_quit = true;
                return;
            }
            _ => {}
        }

        match self.screen {
            Screen::Splash => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.screen = Screen::ThemePick;
                }
            }
            Screen::ThemePick => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.theme_cursor = self.theme_cursor.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.theme_cursor = (self.theme_cursor + 1).min(Theme::ALL.len() - 1);
                }
                KeyCode::Enter => {
                    self.theme = Theme::ALL[self.theme_cursor];
                    self.start_profiling();
                }
                _ => {}
            },
            Screen::Profiling => {} // non-interactive; advances on its own
            Screen::Ollama => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.screen = Screen::Main;
                }
            }
            Screen::Main => self.on_key_main(key),
        }
    }

    fn on_key_main(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.tab = (self.tab + 1) % TABS.len();
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.tab = (self.tab + TABS.len() - 1) % TABS.len();
            }
            KeyCode::Char(c @ '1'..='3') => {
                self.tab = (c as usize - '1' as usize).min(TABS.len() - 1);
            }
            KeyCode::Down | KeyCode::Char('j') if self.tab == 0 => {
                if !self.recs.is_empty() {
                    self.rec_cursor = (self.rec_cursor + 1).min(self.recs.len() - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') if self.tab == 0 => {
                self.rec_cursor = self.rec_cursor.saturating_sub(1);
            }
            KeyCode::Home if self.tab == 0 => self.rec_cursor = 0,
            KeyCode::End if self.tab == 0 => {
                self.rec_cursor = self.recs.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    pub fn selected_rec(&self) -> Option<&Recommendation> {
        self.recs.get(self.rec_cursor)
    }
}

/// Run the real detection + rating pipeline. Returns a `String` error (rather
/// than `anyhow::Error`) so it can cross the thread boundary through the channel.
fn load_everything() -> Result<Loaded, String> {
    let profile = hardware::profile().map_err(|e| format!("hardware profiling failed: {e}"))?;
    let catalog = catalog::load_bundled().map_err(|e| format!("loading catalog failed: {e}"))?;
    let recs = recommend::rate_all(&profile, &catalog, &[]);
    Ok(Loaded {
        profile,
        recs,
        catalog_count: catalog.models.len(),
        ollama: detect_ollama(),
    })
}

/// Is `ollama` on PATH and runnable? Used only to show a friendly nudge — the
/// ratings don't need it.
fn detect_ollama() -> Ollama {
    match Command::new("ollama").arg("--version").output() {
        Ok(out) if out.status.success() => {
            // `ollama --version` may print a "could not connect" warning line to
            // stdout when the daemon is down; pull just the version line.
            let text = String::from_utf8_lossy(&out.stdout);
            let version = text
                .lines()
                .find(|l| l.contains("version is"))
                .and_then(|l| l.rsplit("version is").next())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "installed".into());
            Ollama::Present(version)
        }
        _ => Ollama::Absent,
    }
}

/// Entry point: take over the terminal, run the event loop, and always restore
/// the terminal on the way out (even on error/panic path via `ratatui::restore`).
pub fn run() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut app = App::default();
    let mut last = Instant::now();

    while !app.should_quit {
        terminal.draw(|f| render::draw(f, &app))?;

        let timeout = TICK.saturating_sub(last.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }
        if last.elapsed() >= TICK {
            app.on_tick();
            last = Instant::now();
        }
    }
    Ok(())
}

/// Render the current UI to a fixed-size in-memory buffer and return it as text.
/// Used by `fitllm tui --selftest` to verify layout without a live terminal
/// (handy in CI and on headless hosts). Runs detection synchronously first so
/// the Main screen shows real data.
pub fn selftest(width: u16, height: u16, screen: Screen, tab: usize) -> Result<String> {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = App::default();
    if let Ok(loaded) = load_everything() {
        app.profile = Some(loaded.profile);
        app.recs = loaded.recs;
        app.catalog_count = loaded.catalog_count;
        app.ollama = Some(loaded.ollama);
    }
    app.screen = screen;
    app.tab = tab.min(TABS.len() - 1);
    app.tick = 3;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render::draw(f, &app))?;
    Ok(buffer_to_string(terminal.backend().buffer()))
}

/// Flatten a ratatui test buffer into printable text (symbols only; styles are
/// dropped). Good enough to eyeball alignment and content.
fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn onboarding_walks_splash_to_profiling() {
        let mut app = App::default();
        assert_eq!(app.screen, Screen::Splash);

        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.screen, Screen::ThemePick);

        // Move down to the second theme and select it.
        app.on_key(key(KeyCode::Down));
        assert_eq!(app.theme_cursor, 1);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.screen, Screen::Profiling);
        assert_eq!(app.theme, Theme::ALL[1]);
    }

    #[test]
    fn theme_cursor_is_clamped() {
        let mut app = App::default();
        app.screen = Screen::ThemePick;
        for _ in 0..10 {
            app.on_key(key(KeyCode::Down));
        }
        assert_eq!(app.theme_cursor, Theme::ALL.len() - 1);
        for _ in 0..10 {
            app.on_key(key(KeyCode::Up));
        }
        assert_eq!(app.theme_cursor, 0);
    }

    #[test]
    fn main_tabs_wrap_and_cursor_clamps() {
        let mut app = App::default();
        app.screen = Screen::Main;
        app.recs = load_everything().expect("load").recs;
        assert!(!app.recs.is_empty(), "catalog should yield recommendations");

        // Tabs wrap in both directions.
        app.on_key(key(KeyCode::Tab));
        assert_eq!(app.tab, 1);
        app.on_key(key(KeyCode::BackTab));
        assert_eq!(app.tab, 0);
        app.on_key(key(KeyCode::BackTab));
        assert_eq!(app.tab, TABS.len() - 1);

        // Selection can't run off the ends of the list.
        app.tab = 0;
        for _ in 0..app.recs.len() + 5 {
            app.on_key(key(KeyCode::Down));
        }
        assert_eq!(app.rec_cursor, app.recs.len() - 1);
        app.on_key(key(KeyCode::Home));
        assert_eq!(app.rec_cursor, 0);
    }

    #[test]
    fn quit_keys_work() {
        let mut app = App::default();
        app.screen = Screen::Main;
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);

        let mut app = App::default();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn every_screen_renders_the_brand() {
        for screen in [
            Screen::Splash,
            Screen::ThemePick,
            Screen::Profiling,
            Screen::Ollama,
            Screen::Main,
        ] {
            let out = selftest(100, 30, screen, 0).expect("render");
            // The footer mascot appears on every screen.
            assert!(out.contains("(o.o)~") || out.contains("(-.-)~"),
                "screen {screen:?} produced unexpected output:\n{out}");
        }
    }

    #[test]
    fn llama_frames_are_fixed_width() {
        // Every line of every frame must be the same visual width or the mascot
        // jitters as it animates.
        let width = llama::frame(0).lines[0].chars().count();
        for tick in 0..60u64 {
            for line in llama::frame(tick).lines {
                assert_eq!(line.chars().count(), width, "tick {tick}: '{line}'");
            }
        }
    }
}
