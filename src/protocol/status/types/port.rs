use crate::protocol::{runtime::PortRuntimeHandle, status::types, tty::PortExtra};
use chrono::{DateTime, Local};

/// A single log entry associated with a specific port.
#[derive(Debug, Clone)]
pub struct PortLogEntry {
    pub when: DateTime<Local>,
    pub raw: String,
    pub parsed: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PortOwner {
    Runtime(PortRuntimeHandle),
    CliSubprocess(PortSubprocessInfo),
}

#[derive(Debug, Clone)]
pub struct PortSubprocessInfo {
    pub mode: PortSubprocessMode,
    pub ipc_socket_name: String,
    pub pid: Option<u32>,
    pub data_source_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSubprocessMode {
    SlaveListen,
    SlavePoll,
    MasterProvide,
}

#[derive(Debug, Clone)]
pub enum PortState {
    Free,
    OccupiedByThis { owner: PortOwner },
    OccupiedByOther,
}

impl PortState {
    pub fn owner(&self) -> Option<&PortOwner> {
        match self {
            PortState::OccupiedByThis { owner } => Some(owner),
            _ => None,
        }
    }

    pub fn owner_mut(&mut self) -> Option<&mut PortOwner> {
        match self {
            PortState::OccupiedByThis { owner } => Some(owner),
            _ => None,
        }
    }

    pub fn runtime_handle(&self) -> Option<&PortRuntimeHandle> {
        match self {
            PortState::OccupiedByThis {
                owner: PortOwner::Runtime(handle),
            } => Some(handle),
            _ => None,
        }
    }

    pub fn runtime_handle_mut(&mut self) -> Option<&mut PortRuntimeHandle> {
        match self {
            PortState::OccupiedByThis {
                owner: PortOwner::Runtime(handle),
            } => Some(handle),
            _ => None,
        }
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
