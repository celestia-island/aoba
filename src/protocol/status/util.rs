use crate::protocol::status::types::port::PortData;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;

/// Helper to read port data with a read lock
pub fn with_port_read<T, F>(port: &Arc<RwLock<PortData>>, f: F) -> Option<T>
where
    F: FnOnce(&RwLockReadGuard<PortData>) -> T,
{
    let guard = port.read();
    Some(f(&guard))
}

/// Helper to write port data with a write lock
pub fn with_port_write<T, F>(port: &Arc<RwLock<PortData>>, f: F) -> Option<T>
where
    F: FnOnce(&mut RwLockWriteGuard<PortData>) -> T,
{
    let mut guard = port.write();
    Some(f(&mut guard))
}

/// Convert current port stations to StationConfig format for IPC
/// This is useful when TUI needs to send the current configuration to CLI
pub fn port_stations_to_config(port_data: &PortData) -> Vec<crate::cli::config::StationConfig> {
    use crate::cli::config_convert::register_items_to_stations;
    use crate::protocol::status::types::port::PortConfig;

    match &port_data.config {
        PortConfig::Modbus { mode, stations } => register_items_to_stations(stations, mode.clone()),
    }
}

/// CRC16 checksum for Modbus
pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            if crc & 0x0001 != 0 {
                crc >>= 1;
                crc ^= 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

/// Create a serial port builder
pub fn sp_new(name: &str, baud: u32) -> serialport::SerialPortBuilder {
    serialport::new(name, baud)
}
