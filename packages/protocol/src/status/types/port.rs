use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::{status::types, tty::PortExtra};

/// Serial port configuration (baud rate, data bits, stop bits, parity)
/// This replaces the runtime SerialConfig that was in the disabled runtime module
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: SerialParity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerialParity {
    None,
    Odd,
    Even,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            baud: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: SerialParity::None,
        }
    }
}

/// A single log entry associated with a specific port.
#[derive(Debug, Clone)]
pub struct PortLogEntry {
    pub when: DateTime<Local>,
    pub raw: String,
    pub parsed: Option<String>,
}

/// Port status indicator shown in the title bar
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortStatusIndicator {
    /// Port is not started (red × symbol)
    NotStarted,
    /// Port is starting/initializing (yellow spinner animation)
    Starting,
    /// Port is running and config is up-to-date (green ● solid dot)
    Running,
    /// Port is running but config has been modified (yellow ○ hollow circle)
    RunningWithChanges,
    /// Config is being saved/sent (green spinner animation, for slave 02/04)
    Saving,
    /// Config is being syncing from CLI (yellow spinner animation)
    Syncing,
    /// Config was just successfully applied (green ✔ checkmark, shown for 3 seconds)
    AppliedSuccess { timestamp: DateTime<Local> },
    /// Port failed to start (red text with error message)
    StartupFailed {
        error_message: String,
        timestamp: DateTime<Local>,
    },
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
}

impl PortState {
    pub fn is_occupied_by_this(&self) -> bool {
        matches!(self, PortState::OccupiedByThis)
    }

    pub fn is_free(&self) -> bool {
        matches!(self, PortState::Free)
    }

    pub fn is_occupied_by_other(&self) -> bool {
        matches!(self, PortState::OccupiedByOther)
    }
}

#[derive(Debug, Clone)]
pub struct PortData {
    pub port_name: String,
    pub port_type: String,
    pub extra: PortExtra,
    pub state: PortState,

    /// CLI subprocess info (only present when state is OccupiedByThis)
    pub subprocess_info: Option<PortSubprocessInfo>,

    /// Serial port configuration (baud rate, data bits, stop bits, parity)
    pub serial_config: SerialConfig,

    pub config: PortConfig,

    pub logs: Vec<PortLogEntry>,
    pub log_auto_scroll: bool,
    pub log_clear_pending: bool,

    /// Status indicator for the title bar
    pub status_indicator: PortStatusIndicator,
    /// Whether the config has been modified since last save
    pub config_modified: bool,
}

impl Default for PortData {
    fn default() -> Self {
        PortData {
            port_name: String::new(),
            port_type: String::new(),
            extra: Default::default(),
            state: PortState::Free,
            subprocess_info: None,
            serial_config: SerialConfig::default(),
            config: PortConfig::default(),
            logs: Vec::new(),
            log_auto_scroll: true,
            log_clear_pending: false,
            status_indicator: PortStatusIndicator::NotStarted,
            config_modified: false,
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
