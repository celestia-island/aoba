//! TUI UI E2E Test Runner
//!
//! This program generates screenshots for tui_e2e tests by preparing
//! global state trees and rendering them.

use anyhow::Result;
use clap::{Parser, Subcommand};
use log::info;
use std::{fs, path::Path, process::Command, time::Duration};
use tokio::time::sleep;

use aoba::tui::status::Status;
use aoba_ci_utils::key_input::ExpectKeyExt;

mod e2e;

#[derive(Parser)]
#[command(name = "tui_ui_e2e")]
#[command(about = "TUI UI E2E Test Runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate screenshots for tui_e2e tests
    GenerateScreenshots {
        /// Optional module name to generate screenshots for (e.g., "common", "single_station/master_modes")
        #[arg(long)]
        module: Option<String>,
    },
}

/// Directory to store rendered screenshots
const SCREENSHOT_DIR: &str = "examples/tui_ui_e2e/screenshots";

/// Ensure the screenshot directory exists
fn ensure_screenshot_dir() -> Result<()> {
    let path = Path::new(SCREENSHOT_DIR);
    if !path.exists() {
        fs::create_dir_all(path)?;
        info!("📁 Created screenshot directory: {}", SCREENSHOT_DIR);
    }
    Ok(())
}

/// Clean up status files from previous test runs
async fn cleanup_status_files() -> Result<()> {
    let status_path = Path::new("/tmp/status.json");
    if status_path.exists() {
        std::fs::remove_file(status_path)?;
        log::debug!("🗑️  Removed existing status.json");
    }
    Ok(())
}

/// Save screen capture with proper directory structure
fn save_screen_capture_with_structure(
    test_name: &str,
    step_name: &str,
    screen_content: &str,
) -> Result<()> {
    // Create module directory structure
    let module_dir = Path::new(SCREENSHOT_DIR).join(test_name);
    if !module_dir.exists() {
        fs::create_dir_all(&module_dir)?;
        info!("📁 Created module directory: {}", module_dir.display());
    }

    let filename = format!("{}.txt", step_name.replace(' ', "_"));
    let filepath = module_dir.join(&filename);

    fs::write(&filepath, screen_content)?;
    info!("💾 Saved screen capture: {}", filepath.display());

    Ok(())
}

/// Render a state tree and capture screenshot
async fn render_state_and_capture(test_name: &str, step_name: &str, status: Status) -> Result<()> {
    info!(
        "🎨 Rendering state for test: {}, step: {}",
        test_name, step_name
    );

    // Clean up previous status files
    cleanup_status_files().await?;

    // Convert status to serializable format and write to file
    let serializable_status = status.to_serializable().await;
    let status_json = serializable_status.to_json()?;
    fs::write("/tmp/status.json", status_json)?;
    info!("📄 Wrote state tree to /tmp/status.json");

    // Spawn TUI process with _tui_ui_test feature
    let screen_content = spawn_tui_and_capture_screen().await?;

    // Save the screenshot with proper directory structure
    ensure_screenshot_dir()?;
    save_screen_capture_with_structure(test_name, step_name, &screen_content)?;

    Ok(())
}

/// Spawn TUI process and capture real terminal output using ci_utils
async fn spawn_tui_and_capture_screen() -> Result<String> {
    use aoba_ci_utils::{spawn_expect_session_with_size, TerminalCapture, TerminalSize};

    info!("🧪 Spawning TUI with debug-screen-capture mode");

    // Build the TUI binary first (no features needed)
    let build_status = Command::new("cargo")
        .args(&["build", "--bin", "aoba"])
        .current_dir("../../")
        .status()?;

    if !build_status.success() {
        return Err(anyhow::anyhow!("Failed to build TUI binary"));
    }

    // Get terminal size dimensions
    let size = TerminalSize::Large;
    let (rows, cols) = size.dimensions();

    // Spawn TUI process using ci_utils expectrl with debug-screen-capture mode
    // Use spawn_expect_session_with_size to get a concrete Session type that implements ExpectSession
    let mut session = spawn_expect_session_with_size(
        &["--tui", "--debug-screen-capture", "--no-config-cache"],
        Some((rows, cols)),
    )?;

    // Create terminal capture with proper size
    let mut cap = TerminalCapture::with_size(size);

    // Wait for TUI to initialize and render
    sleep(Duration::from_secs(3)).await;

    // Capture the screen using ci_utils TerminalCapture (handles ANSI properly)
    let screen_content = cap.capture(&mut session, "tui_screenshot").await?;

    // Send Ctrl+C to gracefully exit TUI (ratatui won't exit on its own)
    session.send_ctrl_c()?;

    // Wait a bit for graceful shutdown
    sleep(Duration::from_secs(1)).await;

    info!(
        "📸 Captured {} characters of terminal output",
        screen_content.len()
    );
    Ok(screen_content)
}

/// Helper to check if a test should run based on module filter
fn should_run_test(test_module: &str, filter: Option<&String>) -> bool {
    match filter {
        None => true, // No filter, run all
        Some(f) => {
            // Support exact match or prefix match
            // e.g., "common" matches "common"
            // e.g., "single_station" matches "single_station/master_modes"
            test_module == f || test_module.starts_with(&format!("{}/", f))
        }
    }
}

/// Generate all screenshots for tui_e2e tests
async fn generate_screenshots(module_filter: Option<String>) -> Result<()> {
    if let Some(ref filter) = module_filter {
        info!("🚀 Starting screenshot generation for module: {}", filter);
    } else {
        info!("🚀 Starting screenshot generation for all modules");
    }

    // Generate common base states
    if should_run_test("common", module_filter.as_ref()) {
        info!("🔄 Generating common base states...");
        render_state_and_capture(
            "common",
            "single_station_master_base",
            e2e::common::create_single_station_master_base_state(),
        )
        .await?;

        render_state_and_capture(
            "common",
            "single_station_slave_base",
            e2e::common::create_single_station_slave_base_state(),
        )
        .await?;

        render_state_and_capture(
            "common",
            "multi_station_master_base",
            e2e::common::create_multi_station_master_base_state(),
        )
        .await?;

        render_state_and_capture(
            "common",
            "multi_station_slave_base",
            e2e::common::create_multi_station_slave_base_state(),
        )
        .await?;
    }

    // Generate single station master mode final states
    if should_run_test("single_station/master_modes", module_filter.as_ref()) {
        info!("🔌 Generating single station master mode final states...");
        render_state_and_capture(
            "single_station/master_modes",
            "tui_master_coils_final",
            e2e::single_station::master_modes::create_tui_master_coils_final_state(),
        )
        .await?;

        render_state_and_capture(
            "single_station/master_modes",
            "tui_master_discrete_inputs_final",
            e2e::single_station::master_modes::create_tui_master_discrete_inputs_final_state(),
        )
        .await?;

        render_state_and_capture(
            "single_station/master_modes",
            "tui_master_holding_registers_final",
            e2e::single_station::master_modes::create_tui_master_holding_registers_final_state(),
        )
        .await?;

        render_state_and_capture(
            "single_station/master_modes",
            "tui_master_input_registers_final",
            e2e::single_station::master_modes::create_tui_master_input_registers_final_state(),
        )
        .await?;
    }

    // Generate single station slave mode final states
    if should_run_test("single_station/slave_modes", module_filter.as_ref()) {
        info!("🔌 Generating single station slave mode final states...");
        render_state_and_capture(
            "single_station/slave_modes",
            "tui_slave_coils_final",
            e2e::single_station::slave_modes::create_tui_slave_coils_final_state(),
        )
        .await?;

        render_state_and_capture(
            "single_station/slave_modes",
            "tui_slave_discrete_inputs_final",
            e2e::single_station::slave_modes::create_tui_slave_discrete_inputs_final_state(),
        )
        .await?;

        render_state_and_capture(
            "single_station/slave_modes",
            "tui_slave_holding_registers_final",
            e2e::single_station::slave_modes::create_tui_slave_holding_registers_final_state(),
        )
        .await?;

        render_state_and_capture(
            "single_station/slave_modes",
            "tui_slave_input_registers_final",
            e2e::single_station::slave_modes::create_tui_slave_input_registers_final_state(),
        )
        .await?;
    }

    // Generate multi station master mode final states
    if should_run_test("multi_station/master_modes", module_filter.as_ref()) {
        info!("🔌 Generating multi station master mode final states...");
        render_state_and_capture(
            "multi_station/master_modes",
            "tui_multi_master_mixed_register_types_final",
            e2e::multi_station::master_modes::create_tui_multi_master_mixed_register_types_final_state(
            ),
        )
        .await?;

        render_state_and_capture(
            "multi_station/master_modes",
            "tui_multi_master_spaced_addresses_final",
            e2e::multi_station::master_modes::create_tui_multi_master_spaced_addresses_final_state(),
        )
        .await?;

        render_state_and_capture(
            "multi_station/master_modes",
            "tui_multi_master_mixed_station_ids_final",
            e2e::multi_station::master_modes::create_tui_multi_master_mixed_station_ids_final_state(),
        )
        .await?;
    }

    // Generate multi station slave mode final states
    if should_run_test("multi_station/slave_modes", module_filter.as_ref()) {
        info!("🔌 Generating multi station slave mode final states...");
        render_state_and_capture(
            "multi_station/slave_modes",
            "tui_multi_slave_mixed_register_types_final",
            e2e::multi_station::slave_modes::create_tui_multi_slave_mixed_register_types_final_state(),
        )
        .await?;

        render_state_and_capture(
            "multi_station/slave_modes",
            "tui_multi_slave_spaced_addresses_final",
            e2e::multi_station::slave_modes::create_tui_multi_slave_spaced_addresses_final_state(),
        )
        .await?;

        render_state_and_capture(
            "multi_station/slave_modes",
            "tui_multi_slave_mixed_station_ids_final",
            e2e::multi_station::slave_modes::create_tui_multi_slave_mixed_station_ids_final_state(),
        )
        .await?;
    }

    info!("✅ All screenshots generated successfully!");
    info!("📁 Screenshots saved to: examples/tui_ui_e2e/screenshots/");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateScreenshots { module } => {
            generate_screenshots(module).await?;
        }
    }

    Ok(())
}
