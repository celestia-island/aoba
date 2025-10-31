//! TUI UI End-to-End Test Harness
//!
//! This crate drives first-layer TUI UI tests that simulate keyboard input and
//! validate updates against a mocked global state. The current implementation
//! provides scaffolding for future test cases so CI can exercise the binary and
//! ensure it stays buildable while the detailed scenarios are developed.

use anyhow::{bail, Result};
use clap::Parser;
use log::{info, warn};
use tokio::time::{sleep, Duration};

/// Command-line arguments for the TUI UI E2E harness.
#[derive(Parser, Debug)]
#[command(name = "tui_ui_e2e")]
#[command(about = "TUI UI E2E test suite", long_about = None)]
struct Args {
    /// Specific module to execute. When omitted all available modules will be listed.
    #[arg(long)]
    module: Option<String>,

    /// Enable verbose debug logging for troubleshooting runs.
    #[arg(long)]
    debug: bool,
}

/// Known test modules. The variants currently resolve to lightweight stubs that
/// exercise the harness. Real test logic will replace these stubs as the UI
/// automation layer matures.
const KNOWN_MODULES: &[&str] = &[
    "tui_ui_entry_navigation",
    "tui_ui_modbus_panel_shortcuts",
    "tui_ui_status_bar_rendering",
];

#[tokio::main]
async fn main() -> Result<()> {
    init_logger();
    let args = Args::parse();

    if let Some(module) = args.module.as_deref() {
        run_module(module, args.debug).await
    } else {
        list_available_modules();
        Ok(())
    }
}

fn init_logger() {
    if env_logger::try_init().is_err() {
        warn!("Logger already initialised; continuing without reconfiguration");
    }
}

fn list_available_modules() {
    info!("Available TUI UI E2E modules:");
    for module in KNOWN_MODULES {
        info!("  - {module}");
    }
}

async fn run_module(module: &str, debug: bool) -> Result<()> {
    match module {
        "tui_ui_entry_navigation" => simulate_entry_navigation(debug).await,
        "tui_ui_modbus_panel_shortcuts" => simulate_modbus_panel_shortcuts(debug).await,
        "tui_ui_status_bar_rendering" => simulate_status_bar_rendering(debug).await,
        other => bail!("Unknown module: {other}"),
    }
}

async fn simulate_entry_navigation(debug: bool) -> Result<()> {
    if debug {
        info!("[stub] Validating Entry page navigation interactions");
    }
    sleep(Duration::from_millis(100)).await;
    Ok(())
}

async fn simulate_modbus_panel_shortcuts(debug: bool) -> Result<()> {
    if debug {
        info!("[stub] Validating Modbus panel shortcut handling");
    }
    sleep(Duration::from_millis(100)).await;
    Ok(())
}

async fn simulate_status_bar_rendering(debug: bool) -> Result<()> {
    if debug {
        info!("[stub] Validating status bar rendering reactions");
    }
    sleep(Duration::from_millis(100)).await;
    Ok(())
}
