//! Multi Station Master Mode TUI UI E2E Test States
//!
//! This module provides state constructors for multi station master mode tests,
//! mirroring the test structure from tui_e2e.

use aoba::tui::status::Status;
use aoba_protocol::status::types::{
    modbus::{ModbusConnectionMode, RegisterMode},
    port::{PortConfig, PortData, PortState, SerialConfig},
};

use crate::e2e::common::{create_multi_station_master_base_state, create_register_item};

/// Final state for Test: Mixed Register Types - Station 1 Coils, Station 2 Holding
/// Both stations: ID=1, addr=0x0000, len=10
pub fn create_tui_multi_master_mixed_register_types_final_state() -> Status {
    let mut status = create_multi_station_master_base_state();

    // Add mixed register type stations
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        if let PortConfig::Modbus { stations, .. } = &mut port_data.config {
            stations.push(create_register_item(1, RegisterMode::Coils, 0x0000, 10));
            stations.push(create_register_item(1, RegisterMode::Holding, 0x0000, 10));
        }
    }

    status
}

/// Final state for Test: Spaced Addresses - Station 1 at 0x0000, Station 2 at 0x0100
/// Both stations: ID=1, Holding mode, len=10
pub fn create_tui_multi_master_spaced_addresses_final_state() -> Status {
    let mut status = create_multi_station_master_base_state();

    // Add stations with spaced addresses
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        if let PortConfig::Modbus { stations, .. } = &mut port_data.config {
            stations.push(create_register_item(1, RegisterMode::Holding, 0x0000, 10));
            stations.push(create_register_item(1, RegisterMode::Holding, 0x0100, 10));
        }
    }

    status
}

/// Final state for Test: Mixed Station IDs - Station 1 ID=1, Station 2 ID=2
/// Both stations: Holding mode, addr=0x0000, len=10
pub fn create_tui_multi_master_mixed_station_ids_final_state() -> Status {
    let mut status = create_multi_station_master_base_state();

    // Add stations with mixed station IDs
    if let Some(port_data) = status.ports.map.get_mut("/tmp/vcom1") {
        if let PortConfig::Modbus { stations, .. } = &mut port_data.config {
            stations.push(create_register_item(1, RegisterMode::Holding, 0x0000, 10));
            stations.push(create_register_item(2, RegisterMode::Holding, 0x0000, 10));
        }
    }

    status
}
