use serde::{Deserialize, Serialize};

use crate::protocol::status::read_status;

/// For the config panel we have groups of options separated by blank lines.
/// Define the sizes of each group so view_offset can account for the
/// extra blank rows introduced between groups.
pub const CONFIG_PANEL_GROUP_SIZES: &[usize] = &[3, 4];

/// Cursor trait to unify cursor behaviour across pages.
pub trait Cursor {
    /// Move to previous cursor position
    fn prev(self) -> Self;
    /// Move to next cursor position
    fn next(self) -> Self;
    /// Compute the view offset (number of rows the page should scroll)
    fn view_offset(&self) -> usize;
}

/// UI-oriented enums and small types shared across pages.
/// `EntryCursor` describes the cursor/selection on the main Entry page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryCursor {
    /// Select one of the physical COM ports (index)
    Com { index: usize },
    /// Force a refresh (special entry)
    Refresh,
    /// Create a virtual port entry
    CreateVirtual,
    /// The about page
    About,
}

impl Cursor for EntryCursor {
    fn prev(self) -> Self {
        match self {
            EntryCursor::Com { index } => {
                if index > 0 {
                    EntryCursor::Com { index: index - 1 }
                } else {
                    // Wrap to last special entry
                    EntryCursor::About
                }
            }
            EntryCursor::Refresh => {
                // Go to last COM port if any exist
                let max_port_index =
                    read_status(|status| Ok(status.ports.order.len().saturating_sub(1)))
                        .unwrap_or(0);
                if max_port_index > 0 {
                    EntryCursor::Com {
                        index: max_port_index,
                    }
                } else {
                    EntryCursor::About
                }
            }
            EntryCursor::CreateVirtual => EntryCursor::Refresh,
            EntryCursor::About => EntryCursor::CreateVirtual,
        }
    }

    fn next(self) -> Self {
        match self {
            EntryCursor::Com { index } => {
                let max_port_index =
                    read_status(|status| Ok(status.ports.order.len().saturating_sub(1)))
                        .unwrap_or(0);
                if index < max_port_index {
                    EntryCursor::Com { index: index + 1 }
                } else {
                    EntryCursor::Refresh
                }
            }
            EntryCursor::Refresh => EntryCursor::CreateVirtual,
            EntryCursor::CreateVirtual => EntryCursor::About,
            EntryCursor::About => {
                // Wrap to first COM port if any exist
                if read_status(|status| Ok(!status.ports.order.is_empty())).unwrap_or(false) {
                    EntryCursor::Com { index: 0 }
                } else {
                    EntryCursor::Refresh
                }
            }
        }
    }

    fn view_offset(&self) -> usize {
        match self {
            EntryCursor::Com { index } => *index,
            _ => 0,
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
    DataBits { custom_mode: bool },
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
            ConfigPanelCursor::DataBits { custom_mode: false },
            ConfigPanelCursor::Parity,
            ConfigPanelCursor::StopBits,
            ConfigPanelCursor::ViewCommunicationLog,
        ]
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

impl Cursor for ConfigPanelCursor {
    fn prev(self) -> Self {
        // inline prev logic to avoid extra indirection
        let all = Self::all();
        let current_index = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_index > 0 {
            all[current_index - 1]
        } else {
            all[all.len() - 1]
        }
    }
    fn next(self) -> Self {
        let all = Self::all();
        let current_index = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_index < all.len() - 1 {
            all[current_index + 1]
        } else {
            all[0]
        }
    }
    fn view_offset(&self) -> usize {
        // inline view_offset: return the index of the cursor adjusted for
        // blank rows inserted between groups. We compute the base index
        // then add +1 for each preceding group boundary the index passes.
        let index = Self::all().iter().position(|&c| c == *self).unwrap_or(0);
        // Accumulate extra blank rows introduced before this index
        let mut extra = 0usize;
        let mut running = 0usize;
        for &group_size in CONFIG_PANEL_GROUP_SIZES {
            if index >= running + group_size {
                // there is a blank line after this group
                extra += 1;
                running += group_size;
            } else {
                break;
            }
        }
        index + extra
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

impl Cursor for ModbusDashboardCursor {
    fn prev(self) -> Self {
        let all = Self::all();
        let current_index = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_index > 0 {
            all[current_index - 1]
        } else {
            all[all.len() - 1]
        }
    }
    fn next(self) -> Self {
        let all = Self::all();
        let current_index = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_index < all.len() - 1 {
            all[current_index + 1]
        } else {
            all[0]
        }
    }
    fn view_offset(&self) -> usize {
        Self::all().iter().position(|&c| c == *self).unwrap_or(0)
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

impl Cursor for LogPanelCursor {
    fn prev(self) -> Self {
        let all = Self::all();
        let current_index = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_index > 0 {
            all[current_index - 1]
        } else {
            all[all.len() - 1]
        }
    }
    fn next(self) -> Self {
        let all = Self::all();
        let current_index = all.iter().position(|&c| c == self).unwrap_or(0);
        if current_index < all.len() - 1 {
            all[current_index + 1]
        } else {
            all[0]
        }
    }
    fn view_offset(&self) -> usize {
        Self::all().iter().position(|&c| c == *self).unwrap_or(0)
    }
}
