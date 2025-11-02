/// Detailed multi-station test orchestrator with comprehensive screenshots
///
/// This module provides test orchestrators that capture screenshots at every atomic
/// operation, as requested in feedback. It replaces the high-level orchestrators
/// with detailed step-by-step capture.
use anyhow::Result;

use super::super::{
    config::StationConfig,
    navigation::{configure_stations_with_screenshots, navigate_to_modbus_panel, setup_tui_test},
};
use aoba_ci_utils::*;

/// Run a detailed multi-station master test with comprehensive screenshot capture
///
/// This captures screenshots at every step:
/// 1. Entry page
/// 2. ConfigPanel
/// 3. ModbusDashboard initial
/// 4. After switching to Master mode
/// 5. After creating each station
/// 6. Navigating to each station
/// 7. After editing each field of each station
/// 8. After saving
/// 9. After port enabled
pub async fn run_detailed_multi_master_test(
    port1: &str,
    port2: &str,
    configs: &[StationConfig],
    screenshot_ctx: &SnapshotContext,
) -> Result<()> {
    log::info!(
        "🧪 Running detailed multi-station Master test with {} stations",
        configs.len()
    );

    reset_snapshot_placeholders();

    let is_generation_mode = screenshot_ctx.mode() == ExecutionMode::GenerateScreenshots;

    // Steps 0-1: Setup (captures entry and config_panel)
    let (mut session, mut cap) = setup_tui_test(port1, port2, Some(screenshot_ctx)).await?;

    // Step 2: Navigate to Modbus panel (only in normal mode)
    if !is_generation_mode {
        navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;
        wait_for_tui_page("ModbusDashboard", 10, None).await?;
    }

    // Screenshot: Initial ModbusDashboard
    let state = super::super::state_helpers::create_modbus_dashboard_state(port1);
    let _ = screenshot_ctx
        .capture_or_verify(&mut session, &mut cap, state, "modbus_dashboard_init")
        .await?;

    // Steps 3-N: Configure stations with detailed screenshots (isomorphic workflow)
    configure_stations_with_screenshots(&mut session, &mut cap, port1, configs, screenshot_ctx)
        .await?;

    log::info!("✅ Detailed multi-station Master test completed with comprehensive screenshots");

    // Terminate session (only in normal mode)
    if !is_generation_mode {
        terminate_session(session, "TUI").await?;
    }

    Ok(())
}

/// Run a detailed multi-station slave test with comprehensive screenshot capture
pub async fn run_detailed_multi_slave_test(
    port1: &str,
    port2: &str,
    configs: &[StationConfig],
    screenshot_ctx: &SnapshotContext,
) -> Result<()> {
    log::info!(
        "🧪 Running detailed multi-station Slave test with {} stations",
        configs.len()
    );

    reset_snapshot_placeholders();

    let is_generation_mode = screenshot_ctx.mode() == ExecutionMode::GenerateScreenshots;

    // Steps 0-1: Setup (captures entry and config_panel)
    let (mut session, mut cap) = setup_tui_test(port1, port2, Some(screenshot_ctx)).await?;

    // Step 2: Navigate to Modbus panel (only in normal mode)
    if !is_generation_mode {
        navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;
        wait_for_tui_page("ModbusDashboard", 10, None).await?;
    }

    // Screenshot: Initial ModbusDashboard
    let state = super::super::state_helpers::create_modbus_dashboard_state(port1);
    let _ = screenshot_ctx
        .capture_or_verify(&mut session, &mut cap, state, "modbus_dashboard_init")
        .await?;

    // Steps 3-N: Configure stations with detailed screenshots (isomorphic workflow)
    configure_stations_with_screenshots(&mut session, &mut cap, port1, configs, screenshot_ctx)
        .await?;

    log::info!("✅ Detailed multi-station Slave test completed with comprehensive screenshots");

    // Terminate session (only in normal mode)
    if !is_generation_mode {
        terminate_session(session, "TUI").await?;
    }

    Ok(())
}

/// Run a detailed single-station master test with comprehensive screenshot capture
pub async fn run_detailed_single_master_test(
    port1: &str,
    port2: &str,
    config: StationConfig,
    screenshot_ctx: &SnapshotContext,
) -> Result<()> {
    run_detailed_multi_master_test(port1, port2, &[config], screenshot_ctx).await
}

/// Run a detailed single-station slave test with comprehensive screenshot capture
pub async fn run_detailed_single_slave_test(
    port1: &str,
    port2: &str,
    config: StationConfig,
    screenshot_ctx: &SnapshotContext,
) -> Result<()> {
    run_detailed_multi_slave_test(port1, port2, &[config], screenshot_ctx).await
}
