use chrono::{DateTime, Local};
use serialport::{SerialPort, SerialPortInfo};
use std::{collections::HashMap, time::Duration};

use rmodbus::{
    server::{storage::ModbusStorageSmall, ModbusFrame},
    ModbusProto,
};

use crate::protocol::runtime::{PortRuntimeHandle, RuntimeCommand, RuntimeEvent, SerialConfig};

// ---------- Basic parsed/request data ----------

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
pub struct SubpageForm {
    pub editing: bool,
    pub loop_enabled: bool,
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: serialport::Parity,
    pub cursor: usize,
    pub editing_field: Option<EditingField>,
    pub input_buffer: String,
    pub edit_choice_index: Option<usize>,
    pub edit_confirmed: bool,
    pub registers: Vec<RegisterEntry>,
    pub master_cursor: usize,
    pub master_field_selected: bool,
    pub master_field_editing: bool,
    pub master_edit_field: Option<MasterEditField>,
    pub master_edit_index: Option<usize>,
    pub master_input_buffer: String,
    pub poll_round_index: usize,
    pub in_flight_reg_index: Option<usize>,
    pub global_interval_ms: u64,
    pub global_timeout_ms: u64,
}

impl Default for SubpageForm {
    fn default() -> Self {
        Self {
            editing: false,
            loop_enabled: false,
            baud: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: serialport::Parity::None,
            cursor: 0,
            editing_field: None,
            input_buffer: String::new(),
            edit_choice_index: None,
            edit_confirmed: false,
            registers: Vec::new(),
            master_cursor: 0,
            master_field_selected: false,
            master_field_editing: false,
            master_edit_field: None,
            master_edit_index: None,
            master_input_buffer: String::new(),
            poll_round_index: 0,
            in_flight_reg_index: None,
            global_interval_ms: 1000,
            global_timeout_ms: 3000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterEntry {
    pub role: EntryRole,
    pub slave_id: u8,
    pub mode: RegisterMode,
    pub address: u16,
    pub length: u16,
    pub values: Vec<u16>,
    pub next_poll_at: std::time::Instant,
    pub req_success: u32,
    pub req_total: u32,
    pub pending_requests: Vec<PendingRequest>,
}

impl Default for RegisterEntry {
    fn default() -> Self {
        Self {
            role: EntryRole::Slave,
            slave_id: 1,
            mode: RegisterMode::Holding,
            address: 0,
            length: 1,
            values: vec![0],
            next_poll_at: std::time::Instant::now(),
            req_success: 0,
            req_total: 0,
            pending_requests: Vec::new(),
        }
    }
}

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub func: u8,
    pub address: u16,
    pub count: u16,
    pub sent_at: std::time::Instant,
    pub request: Arc<Mutex<rmodbus::client::ModbusRequest>>,
}

impl PendingRequest {
    pub fn new(
        func: u8,
        address: u16,
        count: u16,
        sent_at: std::time::Instant,
        request: rmodbus::client::ModbusRequest,
    ) -> Self {
        Self {
            func,
            address,
            count,
            sent_at,
            request: Arc::new(Mutex::new(request)),
        }
    }
}

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

// Editing helpers
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
pub struct PerPortState {
    pub subpage_active: bool,
    pub subpage_form: Option<SubpageForm>,
    pub subpage_tab_index: usize,
    pub logs: Vec<crate::protocol::status::LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
    pub app_mode: AppMode,
}

// Status main struct. Some impl methods distributed across modules.
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
    pub subpage_form: Option<SubpageForm>,
    pub subpage_tab_index: usize,
    pub logs: Vec<crate::protocol::status::LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
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
}

impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}
