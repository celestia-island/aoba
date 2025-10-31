//! Single Station Slave Mode TUI UI E2E Test States
//!
//! This module provides state constructors for single station slave mode tests,
//! mirroring the test structure from tui_e2e.

use aoba::tui::status::Status;
use aoba_protocol::status::types::{
    modbus::{ModbusConnectionMode, RegisterMode},
    port::{PortConfig, PortData, PortState, SerialConfig},
};

use crate::e2e::common::{create_register_item, create_single_station_slave_base_state};

/// Final state for Test 01: TUI Slave with Coils mode (0x0100, length 10)
pub fn create_tui_slave_coils_final_state() -> Status {
    let mut status = create_single_station_slave_base_state();

    // Add the Coils station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        if let PortConfig::Modbus { stations, .. } = &mut port_data.config {
            stations.push(create_register_item(1, RegisterMode::Coils, 0x0100, 10));
        }
    }

    status
}

/// Final state for Test 02: TUI Slave with Discrete Inputs mode (0x0200, length 10)
pub fn create_tui_slave_discrete_inputs_final_state() -> Status {
    let mut status = create_single_station_slave_base_state();

    // Add the Discrete Inputs station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        if let PortConfig::Modbus { stations, .. } = &mut port_data.config {
            stations.push(create_register_item(
                1,
                RegisterMode::DiscreteInputs,
                0x0200,
                10,
            ));
        }
    }

    status
}

/// Final state for Test 03: TUI Slave with Holding Registers mode (0x0300, length 10)
pub fn create_tui_slave_holding_registers_final_state() -> Status {
    let mut status = create_single_station_slave_base_state();

    // Add the Holding Registers station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        if let PortConfig::Modbus { stations, .. } = &mut port_data.config {
            stations.push(create_register_item(1, RegisterMode::Holding, 0x0300, 10));
        }
    }

    status
}

/// Final state for Test 04: TUI Slave with Input Registers mode (0x0400, length 10)
pub fn create_tui_slave_input_registers_final_state() -> Status {
    let mut status = create_single_station_slave_base_state();

    // Add the Input Registers station configuration
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        if let PortConfig::Modbus { stations, .. } = &mut port_data.config {
            stations.push(create_register_item(1, RegisterMode::Input, 0x0400, 10));
        }
    }

    status
}
