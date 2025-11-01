/// TUI E2E tests for multi-station (2 stations) Slave mode configurations
///
/// Tests TUI acting as Modbus Slave with multiple stations configured.
use anyhow::Result;

use super::super::common::{make_station_config, run_detailed_multi_slave_test, RegisterMode};
use aoba_ci_utils::{ExecutionMode, ScreenshotContext};

/// Test: Mixed Register Types - Station 1 Coils, Station 2 Holding
/// Both stations: ID=1, addr=0x0000, len=10
pub async fn test_tui_multi_slave_mixed_register_types(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Slave with Mixed Register Types");

    let configs = vec![
    ];

    let screenshot_ctx = ScreenshotContext::new(
        execution_mode,
        "tui_multi_slave_mixed_types".into(),
        "default".into(),
    );

    run_detailed_multi_slave_test(port1, port2, &configs, &screenshot_ctx).await
}

/// Test: Spaced Addresses - Station 1 at 0x0000, Station 2 at 0x0100
/// Both stations: ID=1, Holding mode, len=10
pub async fn test_tui_multi_slave_spaced_addresses(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Slave with Spaced Addresses");

    let configs = vec![
    ];

    let screenshot_ctx = ScreenshotContext::new(
        execution_mode,
        "tui_multi_slave_spaced_addresses".into(),
        "default".into(),
    );

    run_detailed_multi_slave_test(port1, port2, &configs, &screenshot_ctx).await
}

/// Test: Mixed Station IDs - Station 1 ID=1, Station 2 ID=2
/// Both stations: Holding mode, addr=0x0000, len=10
pub async fn test_tui_multi_slave_mixed_station_ids(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Slave with Mixed Station IDs");

    let configs = vec![
    ];

    let screenshot_ctx = ScreenshotContext::new(
        execution_mode,
        "tui_multi_slave_mixed_ids".into(),
        "default".into(),
    );

    run_detailed_multi_slave_test(port1, port2, &configs, &screenshot_ctx).await
}
