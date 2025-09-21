use crate::protocol::status::types::port::PortData;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub fn with_port_read<T, F>(port: &Arc<RwLock<PortData>>, f: F) -> Option<T>
where
    F: FnOnce(&RwLockReadGuard<PortData>) -> T,
{
    match port.read() {
        Ok(guard) => Some(f(&guard)),
        Err(_) => None,
    }
}

pub fn with_port_write<T, F>(port: &Arc<RwLock<PortData>>, f: F) -> Option<T>
where
    F: FnOnce(&mut RwLockWriteGuard<PortData>) -> T,
{
    match port.write() {
        Ok(mut guard) => Some(f(&mut guard)),
        Err(_) => None,
    }
}

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

pub fn sp_new(name: &str, baud: u32) -> serialport::SerialPortBuilder {
    serialport::new(name, baud)
}
