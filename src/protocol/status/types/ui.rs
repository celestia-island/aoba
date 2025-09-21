use crate::protocol::status::types::cursor;
use serde::{Deserialize, Serialize};

// Re-export cursor types so existing `types::ui::EntryCursor` style paths keep working.
pub use crate::protocol::status::types::cursor::{
    ConfigPanelCursor, EntryCursor, LogPanelCursor, ModbusDashboardCursor,
};

use std::fmt;

/// A small enum used to represent temporary input buffers across the UI.
/// - `None` means there's no active temporary buffer
/// - `Index(n)` records a selected index for selectors
/// - `String(bytes)` stores raw bytes when editing free-form input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputRawBuffer {
    None,
    Index(usize),
    String(Vec<u8>),
}

impl Default for InputRawBuffer {
    fn default() -> Self {
        InputRawBuffer::None
    }
}

impl fmt::Display for InputRawBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputRawBuffer::None => write!(f, ""),
            InputRawBuffer::Index(i) => write!(f, "{}", i),
            InputRawBuffer::String(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => write!(f, "{}", s),
                Err(_) => write!(f, "{:?}", bytes),
            },
        }
    }
}

impl From<usize> for InputRawBuffer {
    fn from(i: usize) -> Self {
        InputRawBuffer::Index(i)
    }
}

/// Special entries that appear after the serial ports list in the Entry page.
/// Kept as a UI enum so other UI modules can reference the same canonical variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialEntry {
    Refresh,
    ManualSpecify,
    About,
}

impl SpecialEntry {
    pub const fn all() -> &'static [SpecialEntry] {
        &[
            SpecialEntry::Refresh,
            SpecialEntry::ManualSpecify,
            SpecialEntry::About,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Ascii,
    Hex,
}

// AppMode is a small UI-oriented enum (moved here from modbus.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppMode {
    #[default]
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

/// Snapshot structs for page-specific state to be passed into render/handler
/// functions so callers don't need to inspect the whole `Status` repeatedly.
#[derive(Debug, Clone)]
pub struct ModbusConfigStatus {
    pub selected_port: usize,

    pub edit_active: bool,
    pub edit_port: Option<String>,
    pub edit_field_index: usize,
    pub edit_field_key: Option<String>,
    pub edit_buffer: String,
    pub edit_cursor_pos: usize,
}

#[derive(Debug, Clone)]
pub struct ModbusDashboardStatus {
    pub selected_port: usize,

    pub cursor: usize,
    pub editing_field: Option<crate::protocol::status::types::modbus::EditingField>,
    pub input_buffer: String,
    pub edit_choice_index: Option<usize>,
    pub edit_confirmed: bool,

    pub master_cursor: usize,
    pub master_field_selected: bool,
    pub master_field_editing: bool,
    pub master_edit_field: Option<crate::protocol::status::types::modbus::MasterEditField>,
    pub master_edit_index: Option<usize>,
    pub master_input_buffer: String,
    pub poll_round_index: usize,
    pub in_flight_reg_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ModbusLogStatus {
    pub selected_port: usize,
}

#[derive(Debug, Clone)]
pub struct AboutStatus {
    pub view_offset: usize,
}

#[derive(Debug, Clone)]
pub struct EntryStatus {
    pub cursor: Option<cursor::EntryCursor>,
}
