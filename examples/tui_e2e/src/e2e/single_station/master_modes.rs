/// TUI E2E tests for single-station Master mode with different register modes
///
/// Tests TUI acting as Modbus Master (server) with E2E process as CLI Slave (client).
use anyhow::Result;

use super::super::common::{run_single_station_master_test, RegisterMode, StationConfig};

/// Test 01: TUI Master with Coils mode (0x0000, length 10)
pub async fn test_tui_master_coils(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Coils mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::Coils,
        start_address: 0x0000,
        register_count: 10,
        is_master: true,
        register_values: None, // Will be populated with test data
    };

    run_single_station_master_test(port1, port2, config).await
}

/// Test 02: TUI Master with Discrete Inputs/Writable Coils mode (0x0010, length 10)
pub async fn test_tui_master_discrete_inputs(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Discrete Inputs mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::DiscreteInputs,
        start_address: 0x0010,
        register_count: 10,
        is_master: true,
        register_values: None, // Will be populated with test data
    };

    run_single_station_master_test(port1, port2, config).await
}

/// Test 03: TUI Master with Holding Registers mode (0x0020, length 10)
pub async fn test_tui_master_holding_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Holding Registers mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::Holding,
        start_address: 0x0020,
        register_count: 10,
        is_master: true,
        register_values: None, // Will be populated with test data
    };

    run_single_station_master_test(port1, port2, config).await
}

/// Test 04: TUI Master with Input Registers/Writable Registers mode (0x0030, length 10)
pub async fn test_tui_master_input_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Master with Input Registers mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::Input,
        start_address: 0x0030,
        register_count: 10,
        is_master: true,
        register_values: None, // Will be populated with test data
    };

    run_single_station_master_test(port1, port2, config).await
}
