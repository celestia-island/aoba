/// TUI E2E tests for multi-station (2 stations) Master mode configurations
///
/// Tests TUI acting as Modbus Master with multiple stations configured.
use anyhow::Result;

use super::super::common::{make_station_config, run_detailed_multi_master_test, RegisterMode};
use aoba_ci_utils::{ExecutionMode, ScreenshotContext};

/// Test: Mixed Register Types - Station 1 Coils, Station 2 Holding
/// Both stations: ID=1, addr=0x0000, len=10
pub async fn test_tui_multi_master_mixed_register_types(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Master with Mixed Register Types");

    let master_configs = vec![
        make_station_config(1, RegisterMode::Coils, 0x0000, 10, true, None),
        make_station_config(1, RegisterMode::Holding, 0x0000, 10, true, None),
    ];

    let screenshot_ctx = ScreenshotContext::new(
        execution_mode,
        "multi_station/master_modes/mixed_types".into(),
        "default".into(),
    );

    run_detailed_multi_master_test(port1, port2, &master_configs, &screenshot_ctx).await
}

/// Test: Spaced Addresses - Station 1 at 0x0000, Station 2 at 0x0100
/// Both stations: ID=1, Holding mode, len=10
pub async fn test_tui_multi_master_spaced_addresses(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Master with Spaced Addresses");

    let master_configs = vec![
        make_station_config(1, RegisterMode::Holding, 0x0000, 10, true, None),
        make_station_config(1, RegisterMode::Holding, 0x0100, 10, true, None),
    ];

    let screenshot_ctx = ScreenshotContext::new(
        execution_mode,
        "multi_station/master_modes/spaced_addresses".into(),
        "default".into(),
    );

    run_detailed_multi_master_test(port1, port2, &master_configs, &screenshot_ctx).await
}

/// Test: Mixed Station IDs - Station 1 ID=1, Station 2 ID=2
/// Both stations: Holding mode, addr=0x0000, len=10
pub async fn test_tui_multi_master_mixed_station_ids(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Master with Mixed Station IDs");

    let master_configs = vec![
        make_station_config(1, RegisterMode::Holding, 0x0000, 10, true, None),
        make_station_config(2, RegisterMode::Holding, 0x0000, 10, true, None),
    ];

    let screenshot_ctx = ScreenshotContext::new(
        execution_mode,
        "multi_station/master_modes/mixed_ids".into(),
        "default".into(),
    );

    run_detailed_multi_master_test(port1, port2, &master_configs, &screenshot_ctx).await
}
