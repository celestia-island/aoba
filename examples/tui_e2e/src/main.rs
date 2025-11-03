//! TUI E2E Test Framework - New TOML-based workflow system
//!
//! This is a complete refactoring of the TUI E2E test framework to use
//! declarative TOML workflows instead of imperative Rust code.
//!
//! # Architecture
//!
//! ## Workflow Files
//!
//! Test workflows are defined in TOML files under `workflow/**/*.toml`:
//! - `single_station/master/*.toml` - Single station master mode tests
//! - `single_station/slave/*.toml` - Single station slave mode tests
//! - `multi_station/master/*.toml` - Multi-station master mode tests
//! - `multi_station/slave/*.toml` - Multi-station slave mode tests
//!
//! ## Execution Modes
//!
//! The framework supports two execution modes:
//!
//! ### 1. Rendering Test Mode (`--screen-capture-only`)
//!
//! In this mode, the framework:
//! - Ignores all `key` actions (keyboard input)
//! - Executes `mock_*` actions to manipulate global state
//! - Verifies screen output matches expected patterns
//! - Tests that the TUI renders correctly given specific state
//!
//! ### 2. Drill-Down Test Mode (default)
//!
//! In this mode, the framework:
//! - Executes all `key` actions against a live TUI process
//! - Ignores `mock_*` actions (tests real TUI behavior)
//! - Verifies screen output after each interaction
//! - Treats TUI as a black box, testing end-to-end behavior
//!
//! ## Workflow Format
//!
//! See `workflow/single_station/master/coils.toml` for a complete example.

mod workflow;
mod executor;
mod parser;
mod placeholder;
mod mock_state;

pub use workflow::*;
pub use executor::*;
pub use parser::*;
pub use placeholder::*;
pub use mock_state::*;

use anyhow::Result;
use clap::Parser;

/// TUI E2E test suite with TOML-based workflows
#[derive(Parser, Debug)]
#[command(name = "tui_e2e")]
#[command(about = "TUI E2E test suite with TOML workflows", long_about = None)]
struct Args {
    /// Test module to run (e.g., "single_station_master_coils")
    #[arg(long)]
    module: Option<String>,

    /// Virtual serial port 1 path
    #[arg(long, default_value = "/tmp/vcom1")]
    port1: String,

    /// Virtual serial port 2 path
    #[arg(long, default_value = "/tmp/vcom2")]
    port2: String,

    /// Enable debug mode (show detailed logging)
    #[arg(long)]
    debug: bool,

    /// Screen capture only mode (render testing)
    /// In this mode, mock state is manipulated and screen output is verified
    /// without executing keyboard actions against a live TUI.
    #[arg(long)]
    screen_capture_only: bool,

    /// List all available test modules
    #[arg(long)]
    list: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::builder()
        .filter_level(if args.debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .init();

    log::info!("🧪 TUI E2E Test Framework (TOML-based)");

    // Determine execution mode
    let exec_mode = if args.screen_capture_only {
        ExecutionMode::ScreenCaptureOnly
    } else {
        ExecutionMode::DrillDown
    };

    log::info!("🔧 Execution mode: {:?}", exec_mode);
    log::info!("📍 Port configuration: port1={}, port2={}", args.port1, args.port2);

    // Load all available workflows
    let workflows = load_all_workflows()?;
    
    log::info!("✅ Loaded {} workflow definitions", workflows.len());

    // If list flag is set, list all modules and exit
    if args.list {
        log::info!("📋 Available test modules:");
        for (id, workflow) in &workflows {
            log::info!("  - {} ({})", id, workflow.manifest.description);
        }
        return Ok(());
    }

    // If no module specified, show available modules and exit
    let module = match &args.module {
        Some(m) => m.as_str(),
        None => {
            log::info!("📋 Available test modules:");
            log::info!("  Single-Station Master Mode:");
            log::info!("    - single_station_master_coils");
            log::info!("    - single_station_master_discrete_inputs");
            log::info!("    - single_station_master_holding");
            log::info!("    - single_station_master_input");
            log::info!("  Single-Station Slave Mode:");
            log::info!("    - single_station_slave_coils");
            log::info!("    - single_station_slave_discrete_inputs");
            log::info!("    - single_station_slave_holding");
            log::info!("    - single_station_slave_input");
            log::info!("  Multi-Station Master Mode:");
            log::info!("    - multi_station_master_mixed_types");
            log::info!("    - multi_station_master_spaced_addresses");
            log::info!("    - multi_station_master_mixed_ids");
            log::info!("  Multi-Station Slave Mode:");
            log::info!("    - multi_station_slave_mixed_types");
            log::info!("    - multi_station_slave_spaced_addresses");
            log::info!("    - multi_station_slave_mixed_ids");
            log::info!("");
            log::info!("Usage: cargo run --package tui_e2e -- --module <module_name>");
            log::info!("       cargo run --package tui_e2e -- --list");
            return Ok(());
        }
    };

    // Find the requested workflow
    let workflow = workflows.get(module)
        .ok_or_else(|| anyhow::anyhow!("Unknown module: {}", module))?;

    log::info!("🧪 Running module: {}", module);
    log::info!("📝 Description: {}", workflow.manifest.description);

    // Execute the workflow
    let context = ExecutionContext {
        mode: exec_mode,
        port1: args.port1.clone(),
        port2: args.port2.clone(),
        debug: args.debug,
    };

    execute_workflow(&context, workflow).await?;

    log::info!("✅ Module '{}' completed successfully!", module);
    Ok(())
}

/// Load all workflow TOML files
fn load_all_workflows() -> Result<std::collections::HashMap<String, Workflow>> {
    let mut workflows = std::collections::HashMap::new();

    // Single station master workflows
    workflows.insert(
        "single_station_master_coils".to_string(),
        parse_workflow(include_str!("../workflow/single_station/master/coils.toml"))?,
    );
    workflows.insert(
        "single_station_master_discrete_inputs".to_string(),
        parse_workflow(include_str!("../workflow/single_station/master/discrete_inputs.toml"))?,
    );
    workflows.insert(
        "single_station_master_holding".to_string(),
        parse_workflow(include_str!("../workflow/single_station/master/holding.toml"))?,
    );
    workflows.insert(
        "single_station_master_input".to_string(),
        parse_workflow(include_str!("../workflow/single_station/master/input.toml"))?,
    );

    // Single station slave workflows
    workflows.insert(
        "single_station_slave_coils".to_string(),
        parse_workflow(include_str!("../workflow/single_station/slave/coils.toml"))?,
    );
    workflows.insert(
        "single_station_slave_discrete_inputs".to_string(),
        parse_workflow(include_str!("../workflow/single_station/slave/discrete_inputs.toml"))?,
    );
    workflows.insert(
        "single_station_slave_holding".to_string(),
        parse_workflow(include_str!("../workflow/single_station/slave/holding.toml"))?,
    );
    workflows.insert(
        "single_station_slave_input".to_string(),
        parse_workflow(include_str!("../workflow/single_station/slave/input.toml"))?,
    );

    // Multi-station master workflows
    workflows.insert(
        "multi_station_master_mixed_types".to_string(),
        parse_workflow(include_str!("../workflow/multi_station/master/mixed_types.toml"))?,
    );
    workflows.insert(
        "multi_station_master_spaced_addresses".to_string(),
        parse_workflow(include_str!("../workflow/multi_station/master/spaced_addresses.toml"))?,
    );
    workflows.insert(
        "multi_station_master_mixed_ids".to_string(),
        parse_workflow(include_str!("../workflow/multi_station/master/mixed_ids.toml"))?,
    );

    // Multi-station slave workflows
    workflows.insert(
        "multi_station_slave_mixed_types".to_string(),
        parse_workflow(include_str!("../workflow/multi_station/slave/mixed_types.toml"))?,
    );
    workflows.insert(
        "multi_station_slave_spaced_addresses".to_string(),
        parse_workflow(include_str!("../workflow/multi_station/slave/spaced_addresses.toml"))?,
    );
    workflows.insert(
        "multi_station_slave_mixed_ids".to_string(),
        parse_workflow(include_str!("../workflow/multi_station/slave/mixed_ids.toml"))?,
    );

    Ok(workflows)
}
