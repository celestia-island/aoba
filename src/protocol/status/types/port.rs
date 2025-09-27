use chrono::{DateTime, Local};
use std::sync::{Arc, Mutex};

use crate::protocol::{runtime::PortRuntimeHandle, status::types, tty::PortExtra};

/// A single log entry associated with a specific port.
#[derive(Debug, Clone)]
pub struct PortLogEntry {
    pub when: DateTime<Local>,
    pub raw: String,
    pub parsed: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PortState {
    Free,
    OccupiedByThis {
        handle: Option<SerialPortWrapper>,
        runtime: PortRuntimeHandle,
    },
    OccupiedByOther,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct SerialPortWrapper(Arc<Mutex<Box<dyn serialport::SerialPort + Send>>>);

impl std::fmt::Debug for SerialPortWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SerialPortWrapper").finish()
    }
}

impl SerialPortWrapper {
    pub fn new(inner: Arc<Mutex<Box<dyn serialport::SerialPort + Send>>>) -> Self {
        SerialPortWrapper(inner)
    }
}

#[derive(Debug, Clone)]
pub struct PortData {
    pub port_name: String,
    pub port_type: String,
    pub extra: PortExtra,
    pub state: PortState,
    pub config: PortConfig,

    pub logs: Vec<PortLogEntry>,
    pub log_auto_scroll: bool,
    pub log_clear_pending: bool,

    /// Cache for the last ModbusRequest object to avoid recreating it
    pub last_modbus_request: Option<Arc<Mutex<rmodbus::client::ModbusRequest>>>,
}

impl Default for PortData {
    fn default() -> Self {
        PortData {
            port_name: String::new(),
            port_type: String::new(),
            extra: Default::default(),
            state: PortState::Free,
            config: PortConfig::default(),
            logs: Vec::new(),
            log_auto_scroll: true,
            log_clear_pending: false,
            last_modbus_request: None,
        }
    }
}

/// Protocol-specific configuration attached to a serial port. Currently only Modbus
/// is implemented as a placeholder to hold master/slave configuration lists which
/// can be extended later.
#[derive(Debug, Clone)]
pub enum PortConfig {
    Modbus {
        /// Global master/slave mode for this port. All stations in this port
        /// will operate in the same mode.
        mode: types::modbus::ModbusConnectionMode,
        /// Stores logical entries related to Modbus (using RegisterEntry as a
        /// lightweight placeholder for per-endpoint configuration). The connection_mode
        /// field in individual items is now derived from the global mode above.
        stations: Vec<types::modbus::ModbusRegisterItem>,
    },
}

impl Default for PortConfig {
    fn default() -> Self {
        PortConfig::Modbus {
            mode: types::modbus::ModbusConnectionMode::default_master(),
            stations: Vec::new(),
        }
    }
}
