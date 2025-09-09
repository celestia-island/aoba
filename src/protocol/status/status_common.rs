use std::sync::{Arc, Mutex};

use serialport::SerialPort;

use crate::protocol::status::{AppMode, InputMode, LogEntry, SubpageForm};

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
    pub subpage_form: Option<SubpageForm>,
    pub subpage_tab_index: crate::protocol::status::SubpageTab,
    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub log_clear_pending: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
    pub app_mode: AppMode,
    /// Optional Page snapshot for future page-stack migration. Stores the
    /// page corresponding to this port when available.
    pub page: Option<crate::protocol::status::Page>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct SerialPortWrapper(Arc<Mutex<Box<dyn SerialPort + Send>>>);

impl std::fmt::Debug for SerialPortWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SerialPortWrapper").finish()
    }
}
