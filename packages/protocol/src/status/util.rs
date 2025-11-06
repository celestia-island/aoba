use crate::{status::types::modbus::StationConfig, status::types::port::PortData};

// Note: with_port_read and with_port_write have been removed since PortData
// is now stored directly in the TUI status tree without Arc<RwLock<>>.
// Direct access to PortData is now possible through the status tree.

/// Convert current port stations to StationConfig format for IPC
/// This is useful when TUI needs to send the current configuration to CLI
pub fn port_stations_to_config(port_data: &PortData) -> Vec<StationConfig> {
    use crate::config_convert::register_items_to_stations;
    use crate::status::types::port::PortConfig;

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
