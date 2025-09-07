use chrono::{DateTime, Local};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use yuuka::derive_struct;

use rmodbus::server::storage::ModbusStorageSmall;
use serialport::{SerialPort, SerialPortInfo};

use crate::protocol::{runtime::PortRuntimeHandle, status::LogEntry, tty::PortExtra};

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

#[derive(Clone)]
#[allow(dead_code)]
pub struct SerialPortWrapper(Arc<Mutex<Box<dyn SerialPort + Send>>>);

impl std::fmt::Debug for SerialPortWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SerialPortWrapper").finish()
    }
}

derive_struct! {
    pub Status {
        ports: {
            list: Vec<SerialPortInfo>,
            extras: Vec<PortExtra>,
            states: Vec<PortState>,
            handles: Vec<Option<SerialPortWrapper>>,
            runtimes: Vec<Option<PortRuntimeHandle>>,
            about_view_offset: usize,
        },
        ui: {
            selected: usize,
            auto_refresh: bool,
            last_refresh: Option<DateTime<Local>>,
            error: Option<(String, DateTime<Local>)>,
            subpage_active: bool,
            subpage_form: Option<crate::protocol::status::SubpageForm>,
            subpage_tab_index: usize,
            logs: Vec<LogEntry>,
            log_selected: usize,
            log_view_offset: usize,
            log_auto_scroll: bool,
            log_clear_pending: bool,
            input_mode: InputMode = InputMode::Ascii,
            input_editing: bool,
            input_buffer: String,
            app_mode: AppMode = AppMode::Modbus,
            mode_overlay_active: bool,
            mode_overlay_index: usize,
        },
        per_port: {
            states: HashMap<String, PerPortState>,
            slave_contexts: HashMap<String, Arc<Mutex<ModbusStorageSmall>>>,
            pending_sync_port: Option<String>,
        },
        scan: {
            last_scan_info: Vec<String>,
            last_scan_time: Option<DateTime<Local>>,
        },
        busy: {
            busy: bool = false,
            spinner_frame: u8 = 0,
            polling_paused: bool = false,
        },
        toggles: {
            last_port_toggle: Option<std::time::Instant>,
            port_toggle_min_interval_ms: u64 = PORT_TOGGLE_MIN_INTERVAL_MS,
        },
        recent: {
            auto_sent: VecDeque<(Vec<u8>, std::time::Instant)>,
            auto_requests: VecDeque<(Vec<u8>, std::time::Instant)>,
        },
    }
}
