/// TUI E2E tests for single-station Master mode with different register modes
///
/// Tests TUI acting as Modbus Master (server) with E2E process as CLI Slave (client).
use anyhow::Result;

use super::super::common::{make_station_config, run_detailed_single_master_test, RegisterMode};
use aoba_ci_utils::{ExecutionMode, SnapshotContext};

/// Test 01: TUI Master with Coils mode (0x0000, length 10)
pub async fn test_tui_master_coils(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Coils mode");

    let config = make_station_config(1, RegisterMode::Coils, 0x0000, 10, true, None);

    let snapshot_ctx = SnapshotContext::new(
        execution_mode,
        "single_station/master_modes/coils".into(),
        "default".into(),
    );

    run_detailed_single_master_test(port1, port2, config, &snapshot_ctx).await
}

/// Test 02: TUI Master with Discrete Inputs/Writable Coils mode (0x0000, length 10)
pub async fn test_tui_master_discrete_inputs(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Discrete Inputs mode");

    let config = make_station_config(1, RegisterMode::DiscreteInputs, 0x0000, 10, true, None);

    let snapshot_ctx = SnapshotContext::new(
        execution_mode,
        "single_station/master_modes/discrete_inputs".into(),
        "default".into(),
    );

    run_detailed_single_master_test(port1, port2, config, &snapshot_ctx).await
}

/// Test 03: TUI Master with Holding Registers mode (0x0000, length 10)
pub async fn test_tui_master_holding_registers(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Holding Registers mode");

    let config = make_station_config(1, RegisterMode::Holding, 0x0000, 10, true, None);

    let snapshot_ctx = SnapshotContext::new(
        execution_mode,
        "single_station/master_modes/holding".into(),
        "default".into(),
    );

    run_detailed_single_master_test(port1, port2, config, &snapshot_ctx).await
}

/// Test 04: TUI Master with Input Registers/Writable Registers mode (0x0000, length 10)
pub async fn test_tui_master_input_registers(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,
) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Input Registers mode");

    let config = make_station_config(1, RegisterMode::Input, 0x0000, 10, true, None);

    let snapshot_ctx = SnapshotContext::new(
        execution_mode,
        "single_station/master_modes/input".into(),
        "default".into(),
    );

    run_detailed_single_master_test(port1, port2, config, &snapshot_ctx).await
}
