//! State prediction helpers for TUI E2E screenshot generation
//!
//! This module provides helper functions for creating and modifying
//! TUI states incrementally for screenshot generation and verification.
use aoba_ci_utils::{
    apply_state_change, E2EPortState, StateBuilder, TuiModbusMaster, TuiModbusSlave, TuiPage,
    TuiPort, TuiStatus,
};

/// Create a base port with default values
pub fn create_base_port(name: &str) -> TuiPort {
    TuiPort {
        name: name.to_string(),
        enabled: false,
        state: E2EPortState::Free,
        modbus_masters: Vec::new(),
        modbus_slaves: Vec::new(),
        log_count: 0,
    }
}

/// Create initial state on Entry page with no ports
pub fn create_entry_state() -> TuiStatus {
    StateBuilder::new().with_page(TuiPage::Entry).build()
}

/// Create state on ConfigPanel with given port
pub fn create_config_panel_state(port_name: &str) -> TuiStatus {
    StateBuilder::new()
        .with_page(TuiPage::ConfigPanel)
        .add_port(create_base_port(port_name))
        .build()
}

/// Create state on ModbusDashboard with given port
pub fn create_modbus_dashboard_state(port_name: &str) -> TuiStatus {
    StateBuilder::new()
        .with_page(TuiPage::ModbusDashboard)
        .add_port(create_base_port(port_name))
        .build()
}

/// Enable a port in the state
pub fn enable_port(state: TuiStatus) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            port.enabled = true;
            port.state = E2EPortState::OccupiedByThis;
        }
    })
}

/// Add a master station to the first port
pub fn add_master_station(
    state: TuiStatus,
    station_id: u8,
    register_type: &str,
    start_address: u16,
    register_count: usize,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            port.modbus_masters.push(TuiModbusMaster {
                station_id,
                register_type: register_type.to_string(),
                start_address,
                register_count,
                registers: vec![0; register_count], // Initialize with zeros
            });
        }
    })
}

/// Add a slave station to the first port
pub fn add_slave_station(
    state: TuiStatus,
    station_id: u8,
    register_type: &str,
    start_address: u16,
    register_count: usize,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            port.modbus_slaves.push(TuiModbusSlave {
                station_id,
                register_type: register_type.to_string(),
                start_address,
                register_count,
                registers: vec![0; register_count], // Initialize with zeros
            });
        }
    })
}

/// Update a register value for a station
///
/// # Arguments
/// * `state` - Current TUI status
/// * `station_index` - Index of the station (0-based)
/// * `register_index` - Index of the register within the station (0-based)
/// * `value` - New register value
/// * `is_master` - Whether this is a master station (true) or slave station (false)
pub fn update_register_value(
    state: TuiStatus,
    station_index: usize,
    register_index: usize,
    value: u16,
    is_master: bool,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            if is_master {
                if let Some(station) = port.modbus_masters.get_mut(station_index) {
                    // Ensure registers vec is large enough
                    while station.registers.len() <= register_index {
                        station.registers.push(0);
                    }
                    station.registers[register_index] = value;
                }
            } else {
                if let Some(station) = port.modbus_slaves.get_mut(station_index) {
                    // Ensure registers vec is large enough
                    while station.registers.len() <= register_index {
                        station.registers.push(0);
                    }
                    station.registers[register_index] = value;
                }
            }
        }
    })
}

/// Add a station with default values as created by the TUI (Holding registers, count 1)
pub fn add_default_station(state: TuiStatus, is_master: bool) -> TuiStatus {
    let default_station_id = 1;
    let default_register_type = "Holding";
    let default_start_address = 0u16;
    let default_register_count = 1usize;

    if is_master {
        add_master_station(
            state,
            default_station_id,
            default_register_type,
            default_start_address,
            default_register_count,
        )
    } else {
        add_slave_station(
            state,
            default_station_id,
            default_register_type,
            default_start_address,
            default_register_count,
        )
    }
}

/// Update the station ID for a given station
pub fn update_station_id(
    state: TuiStatus,
    station_index: usize,
    station_id: u8,
    is_master: bool,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            if is_master {
                if let Some(station) = port.modbus_masters.get_mut(station_index) {
                    station.station_id = station_id;
                }
            } else if let Some(station) = port.modbus_slaves.get_mut(station_index) {
                station.station_id = station_id;
            }
        }
    })
}

/// Update the register type for a given station
pub fn update_register_type(
    state: TuiStatus,
    station_index: usize,
    register_type: &str,
    is_master: bool,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            if is_master {
                if let Some(station) = port.modbus_masters.get_mut(station_index) {
                    station.register_type = register_type.to_string();
                }
            } else if let Some(station) = port.modbus_slaves.get_mut(station_index) {
                station.register_type = register_type.to_string();
            }
        }
    })
}

/// Update the start address for a given station
pub fn update_start_address(
    state: TuiStatus,
    station_index: usize,
    start_address: u16,
    is_master: bool,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            if is_master {
                if let Some(station) = port.modbus_masters.get_mut(station_index) {
                    station.start_address = start_address;
                }
            } else if let Some(station) = port.modbus_slaves.get_mut(station_index) {
                station.start_address = start_address;
            }
        }
    })
}

/// Update the register count for a given station, resizing register storage as needed
pub fn update_register_count(
    state: TuiStatus,
    station_index: usize,
    register_count: usize,
    is_master: bool,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            if is_master {
                if let Some(station) = port.modbus_masters.get_mut(station_index) {
                    station.register_count = register_count;
                    station.registers.resize(register_count, 0);
                }
            } else if let Some(station) = port.modbus_slaves.get_mut(station_index) {
                station.register_count = register_count;
                station.registers.resize(register_count, 0);
            }
        }
    })
}
