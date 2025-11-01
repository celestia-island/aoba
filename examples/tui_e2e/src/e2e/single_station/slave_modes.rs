/// TUI E2E tests for single-station Slave mode with different register modes
///
/// Tests TUI acting as Modbus Slave with E2E process as CLI Master.
use anyhow::Result;

use super::super::common::{make_station_config, run_detailed_single_slave_test, RegisterMode};
use aoba_ci_utils::{ExecutionMode, ScreenshotContext};

/// Test 01: TUI Slave with Coils mode (0x0000, length 10)
pub async fn test_tui_slave_coils(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Coils mode");

    let config = make_station_config(1, RegisterMode::Coils, 0x0100, 10, false, None);

    let screenshot_ctx =
        ScreenshotContext::new(execution_mode, "tui_slave_coils".into(), "default".into());

    run_detailed_single_slave_test(port1, port2, config, &screenshot_ctx).await
}

/// Test 02: TUI Slave with Discrete Inputs/Writable Coils mode (0x0010, length 10)
pub async fn test_tui_slave_discrete_inputs(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Discrete Inputs mode");

    let config = make_station_config(1, RegisterMode::DiscreteInputs, 0x0200, 10, false, None);

    let screenshot_ctx = ScreenshotContext::new(
        execution_mode,
        "tui_slave_discrete_inputs".into(),
        "default".into(),
    );

    run_detailed_single_slave_test(port1, port2, config, &screenshot_ctx).await
}

/// Test 03: TUI Slave with Holding Registers mode (0x0020, length 10)
pub async fn test_tui_slave_holding_registers(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Holding Registers mode");

    let config = make_station_config(1, RegisterMode::Holding, 0x0300, 10, false, None);

    let screenshot_ctx =
        ScreenshotContext::new(execution_mode, "tui_slave_holding".into(), "default".into());

    run_detailed_single_slave_test(port1, port2, config, &screenshot_ctx).await
}

/// Test 04: TUI Slave with Input Registers/Writable Registers mode (0x0030, length 10)
pub async fn test_tui_slave_input_registers(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Input Registers mode");

    let config = make_station_config(1, RegisterMode::Input, 0x0400, 10, false, None);

    let screenshot_ctx =
        ScreenshotContext::new(execution_mode, "tui_slave_input".into(), "default".into());

    run_detailed_single_slave_test(port1, port2, config, &screenshot_ctx).await
}
