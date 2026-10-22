use super::types::{modbus::StationConfig, port::PortData};

// Note: with_port_read and with_port_write have been removed since PortData
// is now stored directly in the TUI status tree without Arc<RwLock<>>.
// Direct access to PortData is now possible through the status tree.

/// Convert current port stations to `StationConfig` format for IPC
/// This is useful when TUI needs to send the current configuration to CLI
#[must_use]
pub fn port_stations_to_config(port_data: &PortData) -> Vec<StationConfig> {
    use super::super::config_convert::register_items_to_stations;
    use super::types::port::PortConfig;

    match &port_data.config {
        PortConfig::Modbus { mode, stations, .. } => register_items_to_stations(stations, mode),
    }
}

/// CRC16 checksum for Modbus — delegates to the family hub
/// (`hare::protocol::modbus::crc16`, the single source since 2026-10-22;
/// the local implementation this used to carry is retired).
#[must_use]
pub fn crc16_modbus(data: &[u8]) -> u16 {
    hare::protocol::modbus::crc16(data)
}

/// Create a serial port builder
#[must_use]
pub fn sp_new(name: &str, baud: u32) -> serialport::SerialPortBuilder {
    serialport::new(name, baud)
}
