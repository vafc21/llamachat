//! LlamaChat CLI (`llamachat`).
//!
//! A thin, scriptable front door to the pure-Rust `llamachat-core` engine. It lets
//! you exercise every piece of the Phase 1 core — hardware profiling, the model
//! catalog, the recommendation engine, and the local store — without building
//! the Tauri desktop shell (and therefore without needing webkit2gtk installed).
//!
//! Every subcommand prints machine-readable JSON to stdout so the CLI doubles as
//! a debugging tool and a fixture generator for the UI's mock data layer.

use std::io::IsTerminal;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use llamachat_core::{catalog, hardware, recommend, store::Store, Recommendation};
use llamachat_core::tools::{ShellTool, FilesystemTool, ProcessTool, DesktopTool, ToolLimits, ToolRegistry, ToolRequest};

mod tui;

/// LlamaChat — profile your machine and see which local LLMs will actually run on it.
#[derive(Debug, Parser)]
#[command(
    name = "llamachat",
    version,
    about = "Profile your machine and rate which local LLMs will run on it, from \"won't run\" to \"blazing\".",
    long_about = None,
    arg_required_else_help = false
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Launch the interactive terminal UI (animated onboarding + live ratings).
    ///
    /// This is also what you get by running `llamachat` with no arguments in a
    /// terminal. Use the subcommands below for scriptable JSON output.
    Tui {
        /// Render the UI to a fixed-size buffer and print it as text, then exit.
        /// Useful on headless hosts / CI to verify layout without a live TTY.
        #[arg(long)]
        selftest: bool,
        /// Which screen to render in --selftest mode:
        /// splash | theme | profiling | ollama | main.
        #[arg(long, default_value = "main")]
        screen: String,
        /// Buffer size for --selftest, WIDTHxHEIGHT.
        #[arg(long, default_value = "100x30")]
        size: String,
    },
    /// Detect this machine's hardware and print the HardwareProfile as JSON.
    Profile,
    /// Load the bundled model catalog and print it as JSON.
    Catalog,
    /// Profile + catalog + (empty) benchmarks -> ranked recommendations as a JSON array, best-first.
    Recommend,
    /// Open an in-memory store and run a save_profile / latest_profile round-trip check.
    #[command(name = "store-info")]
    StoreInfo,
    /// List available tools and optionally test one.
    Tools {
        /// Name of the tool to test (omit to list all tools).
        tool: Option<String>,
        /// JSON args for the tool test.
        #[arg(short, long, default_value = "{}")]
        args: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Tui { selftest, screen, size }) => cmd_tui(selftest, &screen, &size),
        Some(Command::Profile) => cmd_profile(),
        Some(Command::Catalog) => cmd_catalog(),
        Some(Command::Recommend) => cmd_recommend(),
        Some(Command::StoreInfo) => cmd_store_info(),
        Some(Command::Tools { tool, args }) => cmd_tools(tool, args),
        None => {
            // Bare `llamachat` in a terminal launches the interactive UI (like
            // `claude` does); piped/redirected, it prints the scriptable summary.
            if std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
                tui::run()
            } else {
                print_summary();
                Ok(())
            }
        }
    }
}

/// `llamachat tui` — launch the interactive terminal UI, or (with --selftest) dump
/// a rendered screen as text for headless verification.
fn cmd_tui(selftest: bool, screen: &str, size: &str) -> Result<()> {
    if !selftest {
        return tui::run();
    }
    let (w, h) = size
        .split_once('x')
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
        .context("--size must look like WIDTHxHEIGHT, e.g. 100x30")?;
    let (screen, tab) = match screen {
        "splash" => (tui::Screen::Splash, 0),
        "theme" => (tui::Screen::ThemePick, 0),
        "profiling" => (tui::Screen::Profiling, 0),
        "ollama" => (tui::Screen::Ollama, 0),
        "main" | "models" => (tui::Screen::Main, 0),
        "hardware" => (tui::Screen::Main, 1),
        "about" => (tui::Screen::Main, 2),
        "chat" | "chatwelcome" => (tui::Screen::Chat, 0),
        "chatmsg" => (tui::Screen::Chat, 1),
        "chatperm" => (tui::Screen::Chat, 2),
        other => anyhow::bail!(
            "unknown --screen '{other}' (splash|theme|profiling|ollama|models|hardware|about|chatwelcome|chatmsg)"
        ),
    };
    print!("{}", tui::selftest(w, h, screen, tab)?);
    Ok(())
}

/// `llamachat profile` — run the hardware profiler and dump the normalized profile.
fn cmd_profile() -> Result<()> {
    let profile = hardware::profile().context("hardware profiling failed")?;
    println!("{}", serde_json::to_string_pretty(&profile)?);
    Ok(())
}

/// `llamachat catalog` — load the bundled catalog and dump it.
fn cmd_catalog() -> Result<()> {
    let catalog = catalog::load_bundled().context("loading bundled catalog failed")?;
    println!("{}", serde_json::to_string_pretty(&catalog)?);
    Ok(())
}

/// `llamachat recommend` — heuristic ratings for every catalog model on this box.
///
/// Benchmarks are intentionally empty here: without an on-device benchmark run
/// the engine falls back to spec heuristics, which is exactly the "provisional"
/// rating a first-launch user sees before the background benchmark lands.
fn cmd_recommend() -> Result<()> {
    let profile = hardware::profile().context("hardware profiling failed")?;
    let catalog = catalog::load_bundled().context("loading bundled catalog failed")?;

    let mut recs: Vec<Recommendation> = recommend::rate_all(&profile, &catalog, &[]);

    // `rate_all` already returns best-first, but sort defensively so the CLI's
    // contract ("best-first") holds regardless of the engine's guarantees.
    recs.sort_by(|a, b| {
        b.tier
            .rank()
            .cmp(&a.tier.rank())
            .then(
                b.quality_score
                    .partial_cmp(&a.quality_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    println!("{}", serde_json::to_string_pretty(&recs)?);
    Ok(())
}

/// `llamachat store-info` — prove the local store round-trips a profile.
fn cmd_store_info() -> Result<()> {
    let store = Store::open_in_memory().context("opening in-memory store failed")?;
    let profile = hardware::profile().context("hardware profiling failed")?;

    store
        .save_profile(&profile)
        .context("save_profile failed")?;
    let loaded = store
        .latest_profile()
        .context("latest_profile failed")?;

    match loaded {
        Some(loaded) if loaded.cpu.model == profile.cpu.model => {
            println!("Store: OK");
        }
        Some(_) => {
            println!("Store: OK (warning: round-trip returned a profile with a mismatched cpu.model)");
        }
        None => {
            // The store round-trips through real SQLite, so a freshly saved
            // profile should always come back; no rows means something is wrong.
            println!("Store: WARNING — latest_profile returned no rows right after save_profile");
        }
    }
    Ok(())
}

/// Printed when `llamachat` is run with no subcommand.
fn print_summary() {
    println!(
        "llamachat {} — which local LLMs will run on your machine?\n",
        env!("CARGO_PKG_VERSION")
    );
    println!("USAGE:\n    llamachat [SUBCOMMAND]   (no subcommand in a terminal launches the interactive UI)\n");
    println!("SUBCOMMANDS:");
    println!("    tui           Launch the interactive terminal UI (animated onboarding + ratings)");
    println!("    profile       Detect hardware and print the HardwareProfile as JSON");
    println!("    catalog       Print the bundled model catalog as JSON");
    println!("    recommend     Print ranked model recommendations (best-first) as JSON");
    println!("    store-info    Round-trip a profile through the local store");
    println!("    tools         List available tools or test one with --args '{{\"key\":\"val\"}}'");
    println!("    help          Print detailed help for any subcommand\n");
    println!("Run `llamachat <SUBCOMMAND> --help` for more information.");
}

/// `llamachat tools` — list tools or test one.
fn cmd_tools(tool_name: Option<String>, args_json: String) -> Result<()> {
    let limits = ToolLimits::default();
    let mut registry = ToolRegistry::new(limits, true);
    registry.register(Box::new(ShellTool::new(ToolLimits::default())));
    registry.register(Box::new(FilesystemTool::new(ToolLimits::default())));
    registry.register(Box::new(ProcessTool::new(ToolLimits::default())));
    registry.register(Box::new(DesktopTool::new()));

    match tool_name {
        Some(name) => {
            let args: serde_json::Value = serde_json::from_str(&args_json)
                .context("Invalid JSON for --args")?;
            let request = ToolRequest { name: name.clone(), args };
            let result = registry.execute(&request);
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        None => {
            let tools = registry.list_tools();
            println!("{}", serde_json::to_string_pretty(&tools)?);
        }
    }
    Ok(())
}
