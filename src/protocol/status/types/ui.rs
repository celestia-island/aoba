use serde::{Deserialize, Serialize};

/// UI-oriented enums and small types shared across pages.
/// `EntryCursor` describes the cursor/selection on the main Entry page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryCursor {
    /// Select one of the physical COM ports (index)
    Com { idx: usize },
    /// Force a refresh (special entry)
    Refresh,
    /// Create a virtual port entry
    CreateVirtual,
    /// The about page
    About,
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
    pub cursor: Option<EntryCursor>,
}
