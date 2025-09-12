use std::fmt;
// use serialport::Parity; // Used in SubpageForm fields via full path

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRole {
    Master,
    Slave,
}

impl fmt::Display for EntryRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryRole::Master => write!(f, "Master"),
            EntryRole::Slave => write!(f, "Slave"),
        }
    }
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
pub enum SubpageTab {
    Config,
    Body,
    Log,
}

impl SubpageTab {
    pub fn as_usize(self) -> usize {
        match self {
            SubpageTab::Config => 0,
            SubpageTab::Body => 1,
            SubpageTab::Log => 2,
        }
    }

    pub fn from_usize(idx: usize) -> SubpageTab {
        match idx {
            0 => SubpageTab::Config,
            1 => SubpageTab::Body,
            2 => SubpageTab::Log,
            _ => SubpageTab::Config,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Modbus,
    Mqtt,
}

impl AppMode {
    pub fn as_usize(self) -> usize {
        match self {
            AppMode::Modbus => 0,
            AppMode::Mqtt => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegisterEntry {
    pub slave_id: u8,
    pub role: EntryRole,
    pub mode: RegisterMode,
    pub address: u16,
    pub length: u16,
    pub req_success: u32,
    pub req_total: u32,
    pub next_poll_at: std::time::Instant,
    pub pending_requests: Vec<u8>, // simplified type for now
    pub values: Vec<u16>,          // Register values
}

#[derive(Debug, Clone)]
pub struct SubpageForm {
    pub registers: Vec<RegisterEntry>,
    pub master_cursor: usize,
    pub master_field_selected: bool,
    pub master_field_editing: bool,
    pub master_edit_field: Option<crate::protocol::status::MasterEditField>,
    pub master_edit_index: Option<usize>,
    pub master_input_buffer: String,
    pub cursor: usize,
    pub loop_enabled: bool,
    pub master_passive: Option<bool>,
    pub editing: bool,

    // Configuration fields
    pub editing_field: Option<crate::protocol::status::EditingField>,
    pub input_buffer: String,
    pub edit_choice_index: Option<usize>,
    pub edit_confirmed: bool,

    // Serial configuration
    pub baud: u32,
    pub parity: serialport::Parity,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub global_interval_ms: u64,
    pub global_timeout_ms: u64,
}
