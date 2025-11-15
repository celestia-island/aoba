use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::modbus::{ModbusConnectionMode, ModbusMasterDataSource, ModbusRegisterItem};
use crate::protocol::tty::PortExtra;

/// Serial port configuration (baud rate, data bits, stop bits, parity)
/// This replaces the runtime SerialConfig that was in the disabled runtime module
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: SerialParity,
    /// Request interval time in milliseconds (for Modbus polling)
    pub request_interval_ms: u32,
    /// Timeout waiting time in milliseconds (for Modbus communication)
    pub timeout_ms: u32,
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
            request_interval_ms: 1000,
            timeout_ms: 3000,
        }
    }
}

/// A single log entry associated with a specific port.
#[derive(Debug, Clone)]
pub struct PortLogEntry {
    pub when: DateTime<Local>,
    pub raw: String,
    pub parsed: Option<String>,
    pub metadata: Option<PortLogMetadata>,
}

#[derive(Debug, Clone)]
pub enum PortLogMetadata {
    Lifecycle(PortLifecycleLog),
    Communication(PortCommunicationLog),
    Management(PortManagementLog),
}

#[derive(Debug, Clone)]
pub struct PortLifecycleLog {
    pub phase: PortLifecyclePhase,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortLifecyclePhase {
    Created,
    Shutdown,
    Restarted,
    Failed,
}

#[derive(Debug, Clone)]
pub struct PortCommunicationLog {
    pub direction: PortCommunicationDirection,
    pub role: super::modbus::StationMode,
    pub station_id: Option<u8>,
    pub config_index: Option<u16>,
    pub register_mode: Option<super::modbus::RegisterMode>,
    pub register_start: Option<u16>,
    pub register_end: Option<u16>,
    pub register_quantity: Option<u16>,
    pub payload: Vec<u8>,
    pub parse_error: Option<String>,
    pub success_hint: Option<bool>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortCommunicationDirection {
    Outbound,
    Inbound,
}

#[derive(Debug, Clone)]
pub struct PortManagementLog {
    pub event: PortManagementEvent,
}

#[derive(Debug, Clone)]
pub enum PortManagementEvent {
    StationsUpdate {
        station_count: usize,
        success: bool,
        error: Option<String>,
    },
    ConfigSync {
        mode: super::modbus::StationMode,
        config_index: u16,
        station_id: u8,
        register_mode: super::modbus::RegisterMode,
        address_start: u16,
        address_end: u16,
        success: bool,
        error: Option<String>,
    },
    StateLockRequest {
        requester: String,
    },
    StateLockAck {
        locked: bool,
    },
    Status {
        status: String,
        details: Option<String>,
    },
    LogMessage {
        level: String,
        message: String,
    },
    SubprocessSpawned {
        mode: String,
        pid: Option<u32>,
    },
    SubprocessStopped {
        reason: Option<String>,
    },
    SubprocessExited {
        success: Option<bool>,
        detail: String,
    },
    RuntimeRestart {
        reason: String,
        connection_mode: super::modbus::StationMode,
    },
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
    /// Port is restarting to apply new config (yellow spinner animation with "重启中")
    Restarting,
    /// Config is being saved/sent (green spinner animation, for slave 02/04)
    Saving,
    /// Config is being syncing from CLI (yellow spinner animation)
    Syncing,
    /// Config was just successfully applied (green ✔ checkmark, shown for 3 seconds)
    AppliedSuccess { timestamp: DateTime<Local> },
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
        mode: ModbusConnectionMode,
        /// Optional external data source configuration used in master mode.
        master_source: ModbusMasterDataSource,
        /// Stores logical entries related to Modbus (using RegisterEntry as a
        /// lightweight placeholder for per-endpoint configuration). The connection_mode
        /// field in individual items is now derived from the global mode above.
        stations: Vec<ModbusRegisterItem>,
    },
}

impl Default for PortConfig {
    fn default() -> Self {
        PortConfig::Modbus {
            mode: ModbusConnectionMode::default_master(),
            master_source: ModbusMasterDataSource::default(),
            stations: Vec::new(),
        }
    }
}
