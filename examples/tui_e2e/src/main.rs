//! TUI E2E Test Framework - New TOML-based workflow system
//!
//! This is a complete refactoring of the TUI E2E test framework to use
//! declarative TOML workflows instead of imperative Rust code, with
//! `ratatui::TestBackend` for rendering and IPC-based screen verification.
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
//! ### 1. Screen Capture Mode (`--screen-capture-only`)
//!
//! In this mode, the framework:
//! - Uses `ratatui::TestBackend` to render the TUI
//! - Manipulates global state directly via mock operations
//! - Verifies screen content using TOML `verify` fields
//! - Tests that the TUI renders correctly given specific state
//! - Ignores keyboard input actions
//!
//! ### 2. DrillDown Mode (default)
//!
//! In this mode, the framework:
//! - Spawns a real TUI process with `--debug-ci` flag
//! - Simulates keyboard input events through IPC channel
//! - Receives rendered screen content from TUI via IPC
//! - Executes keyboard actions and verifies state changes
//! - Tests interactive workflows end-to-end
//! - More realistic testing of user interactions
//!
//! ## Workflow Format
//!
//! See `workflow/single_station/master/coils.toml` for a complete example.

mod executor;
mod mock_state;
mod parser;
mod renderer;
mod retry_state_machine;
mod workflow;

pub use executor::*;

// IPC types are imported directly from `aoba_utils`; `ipc` module was removed.
// If you need IPC types, use `use aoba_utils::...` where required.
// public use ipc::*;
pub use mock_state::*;
pub use parser::*;
pub use renderer::*;
pub use workflow::*;

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

    // Standardize locale so downstream screen checks see consistent text.
    std::env::set_var("LANGUAGE", "en_US");
    std::env::set_var("LC_ALL", "en_US.UTF-8");
    std::env::set_var("LANG", "en_US.UTF-8");

    env_logger::builder()
        .filter_level(if args.debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .init();

    log::info!("🧪 TUI E2E Test Framework (TOML-based)");

    // Ensure language resources are loaded using the caller's locale
    aoba::i18n::init_i18n();

    // Determine execution mode
    let exec_mode = if args.screen_capture_only {
        ExecutionMode::ScreenCaptureOnly
    } else {
        ExecutionMode::DrillDown
    };

    log::info!("🔧 Execution mode: {exec_mode:?}");
    log::info!(
        "📍 Port configuration: port1={args_port1}, port2={args_port2}",
        args_port1 = args.port1,
        args_port2 = args.port2
    );

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
    let workflow = workflows
        .get(module)
        .ok_or_else(|| anyhow::anyhow!("Unknown module: {module}"))?;

    log::info!("🧪 Running module: {module}");
    log::info!(
        "📝 Description: {description}",
        description = workflow.manifest.description
    );

    // Check if this is a slave test based on workflow manifest
    let is_slave_test = workflow
        .manifest
        .mode
        .as_ref()
        .map(|m| matches!(m, aoba_protocol::status::types::modbus::StationMode::Slave))
        .unwrap_or(false);

    // Execute the workflow
    let mut context = ExecutionContext {
        mode: exec_mode,
        port1: args.port1.clone(),
        port2: args.port2.clone(),
        debug: args.debug,
        ipc_sender: None, // Will be initialized in DrillDown mode
        is_slave_test,
        in_modbus_panel: false,
    };

    execute_workflow(&mut context, workflow).await?;

    log::info!("✅ Module '{module}' completed successfully!");
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
        parse_workflow(include_str!(
            "../workflow/single_station/master/discrete_inputs.toml"
        ))?,
    );
    workflows.insert(
        "single_station_master_holding".to_string(),
        parse_workflow(include_str!(
            "../workflow/single_station/master/holding.toml"
        ))?,
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
        parse_workflow(include_str!(
            "../workflow/single_station/slave/discrete_inputs.toml"
        ))?,
    );
    workflows.insert(
        "single_station_slave_holding".to_string(),
        parse_workflow(include_str!(
            "../workflow/single_station/slave/holding.toml"
        ))?,
    );
    workflows.insert(
        "single_station_slave_input".to_string(),
        parse_workflow(include_str!("../workflow/single_station/slave/input.toml"))?,
    );

    // Multi-station master workflows
    workflows.insert(
        "multi_station_master_mixed_types".to_string(),
        parse_workflow(include_str!(
            "../workflow/multi_station/master/mixed_types.toml"
        ))?,
    );
    workflows.insert(
        "multi_station_master_spaced_addresses".to_string(),
        parse_workflow(include_str!(
            "../workflow/multi_station/master/spaced_addresses.toml"
        ))?,
    );
    workflows.insert(
        "multi_station_master_mixed_ids".to_string(),
        parse_workflow(include_str!(
            "../workflow/multi_station/master/mixed_ids.toml"
        ))?,
    );

    // Multi-station slave workflows
    workflows.insert(
        "multi_station_slave_mixed_types".to_string(),
        parse_workflow(include_str!(
            "../workflow/multi_station/slave/mixed_types.toml"
        ))?,
    );
    workflows.insert(
        "multi_station_slave_spaced_addresses".to_string(),
        parse_workflow(include_str!(
            "../workflow/multi_station/slave/spaced_addresses.toml"
        ))?,
    );
    workflows.insert(
        "multi_station_slave_mixed_ids".to_string(),
        parse_workflow(include_str!(
            "../workflow/multi_station/slave/mixed_ids.toml"
        ))?,
    );

    // IPC test workflows (removed)

    Ok(workflows)
}
