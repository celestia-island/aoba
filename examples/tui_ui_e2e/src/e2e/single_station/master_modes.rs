//! Single Station Master Mode TUI UI E2E Test States
//!
//! This module provides state constructors for single station master mode tests,
//! mirroring the test structure from tui_e2e.

use aoba::tui::status::Status;
use aoba_protocol::status::types::{modbus::RegisterMode, port::PortConfig};

use crate::e2e::common::{create_register_item, create_single_station_master_base_state};

/// Final state for Test 01: TUI Master with Coils mode (0x0000, length 10)
pub fn create_tui_master_coils_final_state() -> Status {
    let mut status = create_single_station_master_base_state();

    // Add the Coils station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        let PortConfig::Modbus { stations, .. } = &mut port_data.config;
        stations.push(create_register_item(1, RegisterMode::Coils, 0x0000, 10));
    }

    status
}

/// Final state for Test 02: TUI Master with Discrete Inputs mode (0x0010, length 10)
pub fn create_tui_master_discrete_inputs_final_state() -> Status {
    let mut status = create_single_station_master_base_state();

    // Add the Discrete Inputs station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        let PortConfig::Modbus { stations, .. } = &mut port_data.config;
        stations.push(create_register_item(
            1,
            RegisterMode::DiscreteInputs,
            0x0010,
            10,
        ));
    }

    status
}

/// Final state for Test 03: TUI Master with Holding Registers mode (0x0020, length 10)
pub fn create_tui_master_holding_registers_final_state() -> Status {
    let mut status = create_single_station_master_base_state();

    // Add the Holding Registers station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        let PortConfig::Modbus { stations, .. } = &mut port_data.config;
        stations.push(create_register_item(1, RegisterMode::Holding, 0x0020, 10));
    }

    status
}

/// Final state for Test 04: TUI Master with Input Registers mode (0x0030, length 10)
pub fn create_tui_master_input_registers_final_state() -> Status {
    let mut status = create_single_station_master_base_state();

    // Add the Input Registers station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        let PortConfig::Modbus { stations, .. } = &mut port_data.config;
        stations.push(create_register_item(1, RegisterMode::Input, 0x0030, 10));
    }

    status
}
