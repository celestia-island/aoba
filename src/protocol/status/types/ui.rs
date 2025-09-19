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

impl EntryCursor {
    /// Move to the previous cursor position
    pub fn prev(self) -> Self {
        use crate::protocol::status::read_status;

        match self {
            EntryCursor::Com { idx } => {
                if idx > 0 {
                    EntryCursor::Com { idx: idx - 1 }
                } else {
                    // Wrap to last special entry
                    EntryCursor::About
                }
            }
            EntryCursor::Refresh => {
                // Go to last COM port if any exist
                let max_port_idx =
                    read_status(|s| Ok(s.ports.order.len().saturating_sub(1))).unwrap_or(0);
                if max_port_idx > 0 {
                    EntryCursor::Com { idx: max_port_idx }
                } else {
                    EntryCursor::About
                }
            }
            EntryCursor::CreateVirtual => EntryCursor::Refresh,
            EntryCursor::About => EntryCursor::CreateVirtual,
        }
    }

    /// Move to the next cursor position
    pub fn next(self) -> Self {
        use crate::protocol::status::read_status;

        match self {
            EntryCursor::Com { idx } => {
                let max_port_idx =
                    read_status(|s| Ok(s.ports.order.len().saturating_sub(1))).unwrap_or(0);
                if idx < max_port_idx {
                    EntryCursor::Com { idx: idx + 1 }
                } else {
                    EntryCursor::Refresh
                }
            }
            EntryCursor::Refresh => EntryCursor::CreateVirtual,
            EntryCursor::CreateVirtual => EntryCursor::About,
            EntryCursor::About => {
                // Wrap to first COM port if any exist
                if read_status(|s| Ok(!s.ports.order.is_empty())).unwrap_or(false) {
                    EntryCursor::Com { idx: 0 }
                } else {
                    EntryCursor::Refresh
                }
            }
        }
    }
}

/// ConfigPanelCursor describes the cursor/selection in the config panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigPanelCursor {
    /// Enable/Disable port toggle
    EnablePort,
    /// Protocol mode selection (Modbus/MQTT)
    ProtocolMode,
    /// Protocol configuration navigation
    ProtocolConfig,
    /// Baud rate setting
    BaudRate,
    /// Data bits setting
    DataBits,
    /// Parity setting
    Parity,
    /// Stop bits setting
    StopBits,
    /// View communication log
    ViewCommunicationLog,
}

impl ConfigPanelCursor {
    /// Get all cursor variants in order
    pub const fn all() -> &'static [ConfigPanelCursor] {
        &[
            ConfigPanelCursor::EnablePort,
            ConfigPanelCursor::ProtocolMode,
            ConfigPanelCursor::ProtocolConfig,
            ConfigPanelCursor::BaudRate,
            ConfigPanelCursor::DataBits,
            ConfigPanelCursor::Parity,
            ConfigPanelCursor::StopBits,
            ConfigPanelCursor::ViewCommunicationLog,
        ]
    }

    /// Move to the previous cursor position
    pub fn prev(self) -> Self {
        let all = Self::all();
        let current_idx = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_idx > 0 {
            all[current_idx - 1]
        } else {
            all[all.len() - 1] // Wrap to last
        }
    }

    /// Move to the next cursor position
    pub fn next(self) -> Self {
        let all = Self::all();
        let current_idx = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_idx < all.len() - 1 {
            all[current_idx + 1]
        } else {
            all[0] // Wrap to first
        }
    }

    /// Convert to index for compatibility with existing code
    pub fn to_index(self) -> usize {
        Self::all().iter().position(|&c| c == self).unwrap_or(0)
    }

    /// Convert from index for compatibility with existing code
    pub fn from_index(index: usize) -> Self {
        Self::all()
            .get(index)
            .copied()
            .unwrap_or(ConfigPanelCursor::EnablePort)
    }
}

/// ModbusDashboardCursor describes the cursor/selection in the modbus dashboard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusDashboardCursor {
    /// First item in dashboard
    FirstItem,
    // Add more variants as needed for the dashboard
}

impl ModbusDashboardCursor {
    /// Get all cursor variants in order
    pub const fn all() -> &'static [ModbusDashboardCursor] {
        &[ModbusDashboardCursor::FirstItem]
    }

    /// Move to the previous cursor position
    pub fn prev(self) -> Self {
        let all = Self::all();
        let current_idx = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_idx > 0 {
            all[current_idx - 1]
        } else {
            all[all.len() - 1] // Wrap to last
        }
    }

    /// Move to the next cursor position
    pub fn next(self) -> Self {
        let all = Self::all();
        let current_idx = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_idx < all.len() - 1 {
            all[current_idx + 1]
        } else {
            all[0] // Wrap to first
        }
    }

    /// Convert to index for compatibility with existing code
    pub fn to_index(self) -> usize {
        Self::all().iter().position(|&c| c == self).unwrap_or(0)
    }

    /// Convert from index for compatibility with existing code
    pub fn from_index(index: usize) -> Self {
        Self::all()
            .get(index)
            .copied()
            .unwrap_or(ModbusDashboardCursor::FirstItem)
    }
}

/// LogPanelCursor describes the cursor/selection in the log panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogPanelCursor {
    /// First item in log panel
    FirstItem,
    // Add more variants as needed for the log panel
}

impl LogPanelCursor {
    /// Get all cursor variants in order
    pub const fn all() -> &'static [LogPanelCursor] {
        &[LogPanelCursor::FirstItem]
    }

    /// Move to the previous cursor position
    pub fn prev(self) -> Self {
        let all = Self::all();
        let current_idx = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_idx > 0 {
            all[current_idx - 1]
        } else {
            all[all.len() - 1] // Wrap to last
        }
    }

    /// Move to the next cursor position
    pub fn next(self) -> Self {
        let all = Self::all();
        let current_idx = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_idx < all.len() - 1 {
            all[current_idx + 1]
        } else {
            all[0] // Wrap to first
        }
    }

    /// Convert to index for compatibility with existing code
    pub fn to_index(self) -> usize {
        Self::all().iter().position(|&c| c == self).unwrap_or(0)
    }

    /// Convert from index for compatibility with existing code
    pub fn from_index(index: usize) -> Self {
        Self::all()
            .get(index)
            .copied()
            .unwrap_or(LogPanelCursor::FirstItem)
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
    pub cursor: Option<EntryCursor>,
}
