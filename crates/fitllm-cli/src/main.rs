//! FitLLM CLI (`fitllm`).
//!
//! A thin, scriptable front door to the pure-Rust `fitllm-core` engine. It lets
//! you exercise every piece of the Phase 1 core — hardware profiling, the model
//! catalog, the recommendation engine, and the local store — without building
//! the Tauri desktop shell (and therefore without needing webkit2gtk installed).
//!
//! Every subcommand prints machine-readable JSON to stdout so the CLI doubles as
//! a debugging tool and a fixture generator for the UI's mock data layer.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use fitllm_core::{catalog, hardware, recommend, store::Store, Recommendation};
use fitllm_core::tools::{ShellTool, FilesystemTool, ProcessTool, DesktopTool, ToolLimits, ToolRegistry, ToolRequest};

/// FitLLM — profile your machine and see which local LLMs will actually run on it.
#[derive(Debug, Parser)]
#[command(
    name = "fitllm",
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
        Some(Command::Profile) => cmd_profile(),
        Some(Command::Catalog) => cmd_catalog(),
        Some(Command::Recommend) => cmd_recommend(),
        Some(Command::StoreInfo) => cmd_store_info(),
        Some(Command::Tools { tool, args }) => cmd_tools(tool, args),
        None => {
            print_summary();
            Ok(())
        }
    }
}

/// `fitllm profile` — run the hardware profiler and dump the normalized profile.
fn cmd_profile() -> Result<()> {
    let profile = hardware::profile().context("hardware profiling failed")?;
    println!("{}", serde_json::to_string_pretty(&profile)?);
    Ok(())
}

/// `fitllm catalog` — load the bundled catalog and dump it.
fn cmd_catalog() -> Result<()> {
    let catalog = catalog::load_bundled().context("loading bundled catalog failed")?;
    println!("{}", serde_json::to_string_pretty(&catalog)?);
    Ok(())
}

/// `fitllm recommend` — heuristic ratings for every catalog model on this box.
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

/// `fitllm store-info` — prove the local store round-trips a profile.
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

/// Printed when `fitllm` is run with no subcommand.
fn print_summary() {
    println!(
        "fitllm {} — which local LLMs will run on your machine?\n",
        env!("CARGO_PKG_VERSION")
    );
    println!("USAGE:\n    fitllm <SUBCOMMAND>\n");
    println!("SUBCOMMANDS:");
    println!("    profile       Detect hardware and print the HardwareProfile as JSON");
    println!("    catalog       Print the bundled model catalog as JSON");
    println!("    recommend     Print ranked model recommendations (best-first) as JSON");
    println!("    store-info    Round-trip a profile through the local store");
    println!("    tools         List available tools or test one with --args '{{\"key\":\"val\"}}'");
    println!("    help          Print detailed help for any subcommand\n");
    println!("Run `fitllm <SUBCOMMAND> --help` for more information.");
}

/// `fitllm tools` — list tools or test one.
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
