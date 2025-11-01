//! Conversion functions between the new StationConfig format and the internal ModbusRegisterItem format.
//!
//! This module provides bidirectional conversion to maintain compatibility during the transition
//! from the old configuration structure to the new station-based design.

use std::time::Instant;

use crate::status::types::modbus::{
    ModbusConnectionMode, ModbusRegisterItem, RegisterMap, RegisterMode, RegisterRange,
    StationConfig, StationMode,
};

/// Convert a list of StationConfigs to ModbusRegisterItems
///
/// This flattens the hierarchical station structure into individual register items
/// that can be used by the existing runtime code.
pub fn stations_to_register_items(stations: &[StationConfig]) -> Vec<ModbusRegisterItem> {
    let mut items = Vec::new();

    for station in stations {
        // Convert each register range in the station to a ModbusRegisterItem

        // Coils
        for range in &station.map.coils {
            items.push(ModbusRegisterItem {
                station_id: station.station_id,
                register_mode: RegisterMode::Coils,
                register_address: range.address_start,
                register_length: range.length,
                last_values: range.initial_values.clone(),
                req_success: 0,
                req_total: 0,
                next_poll_at: Instant::now(),
                last_request_time: None,
                last_response_time: None,
                pending_requests: Vec::new(),
            });
        }

        // Discrete Inputs
        for range in &station.map.discrete_inputs {
            items.push(ModbusRegisterItem {
                station_id: station.station_id,
                register_mode: RegisterMode::DiscreteInputs,
                register_address: range.address_start,
                register_length: range.length,
                last_values: range.initial_values.clone(),
                req_success: 0,
                req_total: 0,
                next_poll_at: Instant::now(),
                last_request_time: None,
                last_response_time: None,
                pending_requests: Vec::new(),
            });
        }

        // Holding Registers
        for range in &station.map.holding {
            items.push(ModbusRegisterItem {
                station_id: station.station_id,
                register_mode: RegisterMode::Holding,
                register_address: range.address_start,
                register_length: range.length,
                last_values: range.initial_values.clone(),
                req_success: 0,
                req_total: 0,
                next_poll_at: Instant::now(),
                last_request_time: None,
                last_response_time: None,
                pending_requests: Vec::new(),
            });
        }

        // Input Registers
        for range in &station.map.input {
            items.push(ModbusRegisterItem {
                station_id: station.station_id,
                register_mode: RegisterMode::Input,
                register_address: range.address_start,
                register_length: range.length,
                last_values: range.initial_values.clone(),
                req_success: 0,
                req_total: 0,
                next_poll_at: Instant::now(),
                last_request_time: None,
                last_response_time: None,
                pending_requests: Vec::new(),
            });
        }
    }

    items
}

/// Convert a list of ModbusRegisterItems back to StationConfigs
///
/// This groups register items by station ID and organizes them by register type.
/// The mode parameter indicates whether the station should be Master or Slave.
pub fn register_items_to_stations(
    items: &[ModbusRegisterItem],
    mode: ModbusConnectionMode,
) -> Vec<StationConfig> {
    use std::collections::HashMap;

    // Group items by station ID
    let mut station_map: HashMap<u8, RegisterMap> = HashMap::new();

    for item in items {
        let map = station_map.entry(item.station_id).or_default();

        let range = RegisterRange {
            address_start: item.register_address,
            length: item.register_length,
            initial_values: item.last_values.clone(),
        };

        match item.register_mode {
            RegisterMode::Coils => map.coils.push(range),
            RegisterMode::DiscreteInputs => map.discrete_inputs.push(range),
            RegisterMode::Holding => map.holding.push(range),
            RegisterMode::Input => map.input.push(range),
        }
    }

    // Convert to StationConfig list
    let mut stations: Vec<StationConfig> = station_map
        .into_iter()
        .map(|(station_id, map)| StationConfig {
            station_id,
            mode: modbus_connection_mode_to_station_mode(&mode),
            map,
        })
        .collect();

    // Sort by station ID for consistent ordering
    stations.sort_by_key(|s| s.station_id);

    stations
}

/// Convert ModbusConnectionMode to StationMode
pub fn modbus_connection_mode_to_station_mode(mode: &ModbusConnectionMode) -> StationMode {
    match mode {
        ModbusConnectionMode::Master => StationMode::Master,
        ModbusConnectionMode::Slave { .. } => StationMode::Slave,
    }
}

/// Convert StationMode to ModbusConnectionMode
pub fn station_mode_to_modbus_connection_mode(mode: StationMode) -> ModbusConnectionMode {
    match mode {
        StationMode::Master => ModbusConnectionMode::Master,
        StationMode::Slave => ModbusConnectionMode::Slave {
            current_request_at_station_index: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stations_to_register_items() {
        let stations = vec![StationConfig {
            station_id: 1,
            mode: StationMode::Master,
            map: RegisterMap {
                holding: vec![RegisterRange {
                    address_start: 0,
                    length: 10,
                    initial_values: vec![100, 200],
                }],
                coils: vec![RegisterRange {
                    address_start: 100,
                    length: 5,
                    initial_values: vec![],
                }],
                ..Default::default()
            },
        }];

        let items = stations_to_register_items(&stations);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].station_id, 1);
        assert_eq!(items[0].register_mode, RegisterMode::Coils);
        assert_eq!(items[0].register_address, 100);
        assert_eq!(items[1].register_mode, RegisterMode::Holding);
        assert_eq!(items[1].register_address, 0);
    }

    #[test]
    fn test_register_items_to_stations() {
        let items = vec![
            ModbusRegisterItem {
                station_id: 1,
                register_mode: RegisterMode::Holding,
                register_address: 0,
                register_length: 10,
                last_values: vec![100, 200],
                req_success: 0,
                req_total: 0,
                next_poll_at: Instant::now(),
                last_request_time: None,
                last_response_time: None,
                pending_requests: Vec::new(),
            },
            ModbusRegisterItem {
                station_id: 1,
                register_mode: RegisterMode::Coils,
                register_address: 100,
                register_length: 5,
                last_values: vec![],
                req_success: 0,
                req_total: 0,
                next_poll_at: Instant::now(),
                last_request_time: None,
                last_response_time: None,
                pending_requests: Vec::new(),
            },
        ];

        let stations = register_items_to_stations(&items, ModbusConnectionMode::Master);

        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].station_id, 1);
        assert_eq!(stations[0].mode, StationMode::Master);
        assert_eq!(stations[0].map.holding.len(), 1);
        assert_eq!(stations[0].map.coils.len(), 1);
    }
}
