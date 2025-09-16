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

/// TextState is a small helper enum used by UI components for styling decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextState {
    Normal,
    Selected,
    Chosen,
    Editing,
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

// Implement convenient conversions from a reference to `types::Page` so callers
// can obtain page-specific snapshot structs with a single `from(&page)` call.
impl From<&crate::protocol::status::types::Page> for EntryStatus {
    fn from(p: &crate::protocol::status::types::Page) -> Self {
        match p {
            crate::protocol::status::types::Page::Entry { cursor } => {
                EntryStatus { cursor: *cursor }
            }
            other => panic!("Expected Entry page, found: {:?}", other),
        }
    }
}

impl From<&crate::protocol::status::types::Page> for AboutStatus {
    fn from(p: &crate::protocol::status::types::Page) -> Self {
        match p {
            crate::protocol::status::types::Page::About { view_offset } => AboutStatus {
                view_offset: *view_offset,
            },
            other => panic!("Expected About page, found: {:?}", other),
        }
    }
}

impl From<&crate::protocol::status::types::Page> for ModbusConfigStatus {
    fn from(p: &crate::protocol::status::types::Page) -> Self {
        match p {
            crate::protocol::status::types::Page::ModbusConfig {
                selected_port,
                edit_active,
                edit_port,
                edit_field_index,
                edit_field_key,
                edit_buffer,
                edit_cursor_pos,
                ..
            } => ModbusConfigStatus {
                selected_port: *selected_port,
                edit_active: *edit_active,
                edit_port: edit_port.clone(),
                edit_field_index: *edit_field_index,
                edit_field_key: edit_field_key.clone(),
                edit_buffer: edit_buffer.clone(),
                edit_cursor_pos: *edit_cursor_pos,
            },
            other => panic!("Expected ModbusConfig page, found: {:?}", other),
        }
    }
}

impl From<&crate::protocol::status::types::Page> for ModbusDashboardStatus {
    fn from(p: &crate::protocol::status::types::Page) -> Self {
        match p {
            crate::protocol::status::types::Page::ModbusDashboard {
                selected_port,
                cursor,
                editing_field,
                input_buffer,
                edit_choice_index,
                edit_confirmed,
                master_cursor,
                master_field_selected,
                master_field_editing,
                master_edit_field,
                master_edit_index,
                master_input_buffer,
                poll_round_index,
                in_flight_reg_index,
                ..
            } => ModbusDashboardStatus {
                selected_port: *selected_port,
                cursor: *cursor,
                editing_field: *editing_field,
                input_buffer: input_buffer.clone(),
                edit_choice_index: *edit_choice_index,
                edit_confirmed: *edit_confirmed,
                master_cursor: *master_cursor,
                master_field_selected: *master_field_selected,
                master_field_editing: *master_field_editing,
                master_edit_field: master_edit_field.clone(),
                master_edit_index: *master_edit_index,
                master_input_buffer: master_input_buffer.clone(),
                poll_round_index: *poll_round_index,
                in_flight_reg_index: *in_flight_reg_index,
            },
            other => panic!("Expected ModbusDashboard page, found: {:?}", other),
        }
    }
}

impl From<&crate::protocol::status::types::Page> for ModbusLogStatus {
    fn from(p: &crate::protocol::status::types::Page) -> Self {
        match p {
            crate::protocol::status::types::Page::ModbusLog { selected_port, .. } => {
                ModbusLogStatus {
                    selected_port: *selected_port,
                }
            }
            other => panic!("Expected ModbusLog page, found: {:?}", other),
        }
    }
}
