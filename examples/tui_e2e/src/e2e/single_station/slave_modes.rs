use super::super::common::*;
/// TUI E2E tests for single-station Slave mode with different register modes
///
/// Tests TUI acting as Modbus Slave with E2E process as CLI Master.
use anyhow::Result;

/// Test 01: TUI Slave with Coils mode (0x0000, length 10)
pub async fn test_tui_slave_coils(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Coils mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::Coils,
        start_address: 0x0000,
        register_count: 10,
        is_master: false, // Slave mode
        register_values: None,
    };

    run_single_station_slave_test(port1, port2, config).await
}

/// Test 02: TUI Slave with Discrete Inputs/Writable Coils mode (0x0010, length 10)
pub async fn test_tui_slave_discrete_inputs(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Discrete Inputs mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::DiscreteInputs,
        start_address: 0x0010,
        register_count: 10,
        is_master: false, // Slave mode
        register_values: None,
    };

    run_single_station_slave_test(port1, port2, config).await
}

/// Test 03: TUI Slave with Holding Registers mode (0x0020, length 10)
pub async fn test_tui_slave_holding_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Holding Registers mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::Holding,
        start_address: 0x0020,
        register_count: 10,
        is_master: false, // Slave mode
        register_values: None,
    };

    run_single_station_slave_test(port1, port2, config).await
}

/// Test 04: TUI Slave with Input Registers/Writable Registers mode (0x0030, length 10)
pub async fn test_tui_slave_input_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Slave with Input Registers mode");

    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::Input,
        start_address: 0x0030,
        register_count: 10,
        is_master: false, // Slave mode
        register_values: None,
    };

    run_single_station_slave_test(port1, port2, config).await
}
