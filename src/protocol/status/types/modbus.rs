#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRole {
    Master,
    Slave,
}

impl std::fmt::Display for EntryRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubpageTab {
    #[default]
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

// SubpageForm removed — type intentionally deleted to simplify status types.
// If any functionality relied on SubpageForm, consider replacing it with
// a smaller, focused structure in another module or in the UI layer.

// Reusable enums moved here so other modules can reference them via types::modbus::*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterField {
    SlaveId,
    Mode,
    Address,
    Length,
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

// EntryCursor moved to `types::ui` (ui.rs). Keep modbus.rs focused on modbus-specific types.
