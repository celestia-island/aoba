/// TUI E2E tests for multi-station (2 stations) Slave mode configurations
///
/// Tests TUI acting as Modbus Slave with multiple stations configured.

use anyhow::Result;
use super::super::common::*;

/// Test: Mixed Register Types - Station 1 Coils, Station 2 Holding
/// Both stations: ID=1, addr=0x0000, len=10
pub async fn test_tui_multi_slave_mixed_register_types(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Slave with Mixed Register Types");

    let configs = vec![
        StationConfig {
            station_id: 1,
            register_mode: RegisterMode::Coils,
            start_address: 0x0000,
            register_count: 10,
            is_master: false, // Slave mode
            register_values: None,
        },
        StationConfig {
            station_id: 1,
            register_mode: RegisterMode::Holding,
            start_address: 0x0000,
            register_count: 10,
            is_master: false, // Slave mode
            register_values: None,
        },
    ];

    run_multi_station_slave_test(port1, port2, configs).await
}

/// Test: Spaced Addresses - Station 1 at 0x0000, Station 2 at 0x0100
/// Both stations: ID=1, Holding mode, len=10
pub async fn test_tui_multi_slave_spaced_addresses(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Slave with Spaced Addresses");

    let configs = vec![
        StationConfig {
            station_id: 1,
            register_mode: RegisterMode::Holding,
            start_address: 0x0000,
            register_count: 10,
            is_master: false, // Slave mode
            register_values: None,
        },
        StationConfig {
            station_id: 1,
            register_mode: RegisterMode::Holding,
            start_address: 0x0100,
            register_count: 10,
            is_master: false, // Slave mode
            register_values: None,
        },
    ];

    run_multi_station_slave_test(port1, port2, configs).await
}

/// Test: Mixed Station IDs - Station 1 ID=1, Station 2 ID=2
/// Both stations: Holding mode, addr=0x0000, len=10
pub async fn test_tui_multi_slave_mixed_station_ids(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Test: TUI Multi-Slave with Mixed Station IDs");

    let configs = vec![
        StationConfig {
            station_id: 1,
            register_mode: RegisterMode::Holding,
            start_address: 0x0000,
            register_count: 10,
            is_master: false, // Slave mode
            register_values: None,
        },
        StationConfig {
            station_id: 2,
            register_mode: RegisterMode::Holding,
            start_address: 0x0000,
            register_count: 10,
            is_master: false, // Slave mode
            register_values: None,
        },
    ];

    run_multi_station_slave_test(port1, port2, configs).await
}
