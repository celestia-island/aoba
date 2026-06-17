#[cfg(feature = "async-serial")]
pub mod async_serial;
pub mod modbus;
pub mod utils;

pub use crate::protocol::status::types::{modbus::RegisterMode, port::PortType};

/// Check whether a port name refers to a virtual (non-physical) endpoint.
///
/// Virtual ports include IPC channels (UUID format) and HTTP/HTTPS URLs.
/// Physical serial ports (e.g. `/dev/ttyUSB0`, `COM3`) return `false`.
#[must_use]
pub fn is_virtual_port(port_name: &str) -> bool {
    PortType::detect(port_name).is_virtual()
}
