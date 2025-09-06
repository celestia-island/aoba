use chrono::{DateTime, Local};
use rmodbus::server::storage::ModbusStorageSmall;
use serialport::{SerialPort, SerialPortInfo};
use std::collections::{HashMap, VecDeque};

use crate::protocol::runtime::PortRuntimeHandle;
use crate::protocol::status::LogEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRole {
    Master,
    Slave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterMode {
    Coils = 1,
    DiscreteInputs = 2,
    Holding = 3,
    Input = 4,
}
impl RegisterMode {
    pub const fn all() -> &'static [RegisterMode] {
        &[
            Self::Coils,
            Self::DiscreteInputs,
            Self::Holding,
            Self::Input,
        ]
    }
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => Self::Coils,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Ascii,
    Hex,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Modbus,
    Mqtt,
}
impl AppMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Modbus => Self::Mqtt,
            Self::Mqtt => Self::Modbus,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Modbus => "ModBus RTU",
            Self::Mqtt => "MQTT",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
}

pub const PORT_TOGGLE_MIN_INTERVAL_MS: u64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterField {
    SlaveId,
    Mode,
    Address,
    Length,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditingField {
    Loop,
    Baud,
    Parity,
    StopBits,
    DataBits,
    GlobalInterval,
    GlobalTimeout,
    RegisterField { idx: usize, field: RegisterField },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterEditField {
    Role,
    Id,
    Type,
    Start,
    End,
    Counter,
    Value(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub origin: String,
    pub rw: String,
    pub command: String,
    pub slave_id: u8,
    pub address: u16,
    pub length: u16,
}

#[derive(Debug, Clone)]
pub struct PerPortState {
    pub subpage_active: bool,
    pub subpage_form: Option<crate::protocol::status::SubpageForm>,
    pub subpage_tab_index: usize,
    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub log_clear_pending: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
    pub app_mode: AppMode,
}

#[derive(Debug)]
pub struct Status {
    pub ports: Vec<SerialPortInfo>,
    pub port_extras: Vec<crate::protocol::tty::PortExtra>,
    pub port_states: Vec<PortState>,
    pub port_handles: Vec<Option<Box<dyn SerialPort>>>,
    pub port_runtimes: Vec<Option<PortRuntimeHandle>>,
    pub selected: usize,
    pub auto_refresh: bool,
    pub last_refresh: Option<DateTime<Local>>,
    pub error: Option<(String, DateTime<Local>)>,
    pub subpage_active: bool,
    pub subpage_form: Option<crate::protocol::status::SubpageForm>,
    pub subpage_tab_index: usize,
    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub log_clear_pending: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
    pub app_mode: AppMode,
    pub mode_overlay_active: bool,
    pub mode_overlay_index: usize,
    pub(crate) per_port_states: HashMap<String, PerPortState>,
    pub(crate) per_port_slave_contexts: HashMap<String, ModbusStorageSmall>,
    pub last_scan_info: Vec<String>,
    pub last_scan_time: Option<DateTime<Local>>,
    pub busy: bool,
    pub spinner_frame: u8,
    pub polling_paused: bool,
    pub last_port_toggle: Option<std::time::Instant>,
    pub port_toggle_min_interval_ms: u64,
    // recent auto-generated responses (bytes, timestamp) to avoid double-logging when
    // the runtime emits a FrameSent for the same bytes.
    pub(crate) recent_auto_sent: VecDeque<(Vec<u8>, std::time::Instant)>,
    // recent auto-generated request bytes (incoming requests we auto-responded to)
    // stored to implement debounce based on form.global_interval_ms
    pub(crate) recent_auto_requests: VecDeque<(Vec<u8>, std::time::Instant)>,
    // When set, indicates that we should sync current form into the given port's slave context
    pub(crate) pending_sync_port: Option<String>,
}
impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}
