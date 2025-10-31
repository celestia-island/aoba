use serde::{Deserialize, Serialize};

/// For the config panel we have groups of options separated by blank lines.
/// Define the sizes of each group so view_offset can account for the
/// extra blank rows introduced between groups.
pub const CONFIG_PANEL_GROUP_SIZES: &[usize] = &[4, 4];

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
                let max_port_index = crate::tui::status::read_status(|status| {
                    Ok(status.ports.order.len().saturating_sub(1))
                })
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
                let max_port_index = crate::tui::status::read_status(|status| {
                    Ok(status.ports.order.len().saturating_sub(1))
                })
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
                if crate::tui::status::read_status(|status| Ok(!status.ports.order.is_empty()))
                    .unwrap_or(false)
                {
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
            EntryCursor::Refresh => {
                // When scrolling to the last 3 items, add +1 offset
                let ports_count =
                    crate::tui::status::read_status(|status| Ok(status.ports.order.len()))
                        .unwrap_or(0);
                ports_count.saturating_add(1)
            }
            EntryCursor::CreateVirtual => {
                let ports_count =
                    crate::tui::status::read_status(|status| Ok(status.ports.order.len()))
                        .unwrap_or(0);
                ports_count.saturating_add(2)
            }
            EntryCursor::About => {
                let ports_count =
                    crate::tui::status::read_status(|status| Ok(status.ports.order.len()))
                        .unwrap_or(0);
                ports_count.saturating_add(3)
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
            ConfigPanelCursor::ViewCommunicationLog,
            ConfigPanelCursor::BaudRate,
            ConfigPanelCursor::DataBits { custom_mode: false },
            ConfigPanelCursor::Parity,
            ConfigPanelCursor::StopBits,
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
///
/// This cursor carries explicit identity information for the selected element so
/// renderers and input handlers can determine exactly which block and which
/// register (cell) is active without relying on fragile numeric row-to-block
/// conversions. To keep the enum serializable, `mode` is stored as `u8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusDashboardCursor {
    AddLine,
    /// Select the global mode for all stations in this port (Master/Slave)
    ModbusMode,
    StationId {
        index: usize,
    },
    RegisterMode {
        index: usize,
    },
    RegisterStartAddress {
        index: usize,
    },
    RegisterLength {
        index: usize,
    },
    Register {
        slave_index: usize,
        register_index: usize,
    },
}

impl Cursor for ModbusDashboardCursor {
    fn prev(self) -> Self {
        // Build flat ordered list using shared helper to keep behavior consistent
        let mut flat: Vec<ModbusDashboardCursor> = Vec::new();
        flat.push(ModbusDashboardCursor::AddLine);
        flat.push(ModbusDashboardCursor::ModbusMode);

        let items_vec = build_modbus_items_vec();
        for (idx, item) in items_vec.iter().enumerate() {
            flat.push(ModbusDashboardCursor::StationId { index: idx });
            flat.push(ModbusDashboardCursor::RegisterMode { index: idx });
            flat.push(ModbusDashboardCursor::RegisterStartAddress { index: idx });
            flat.push(ModbusDashboardCursor::RegisterLength { index: idx });
            let regs = item.register_length as usize;
            for reg in 0..regs {
                flat.push(ModbusDashboardCursor::Register {
                    slave_index: idx,
                    register_index: reg,
                });
            }
        }

        let cur_pos = flat.iter().position(|c| *c == self).unwrap_or(0);
        if cur_pos == 0 {
            flat[0]
        } else {
            flat[cur_pos - 1]
        }
    }

    fn next(self) -> Self {
        let mut flat: Vec<ModbusDashboardCursor> = Vec::new();
        flat.push(ModbusDashboardCursor::AddLine);
        flat.push(ModbusDashboardCursor::ModbusMode);

        let items_vec = build_modbus_items_vec();
        for (idx, item) in items_vec.iter().enumerate() {
            flat.push(ModbusDashboardCursor::StationId { index: idx });
            flat.push(ModbusDashboardCursor::RegisterMode { index: idx });
            flat.push(ModbusDashboardCursor::RegisterStartAddress { index: idx });
            flat.push(ModbusDashboardCursor::RegisterLength { index: idx });
            let regs = item.register_length as usize;
            for reg in 0..regs {
                flat.push(ModbusDashboardCursor::Register {
                    slave_index: idx,
                    register_index: reg,
                });
            }
        }

        let cur_pos = flat.iter().position(|c| *c == self).unwrap_or(0);
        if cur_pos + 1 >= flat.len() {
            flat[flat.len() - 1]
        } else {
            flat[cur_pos + 1]
        }
    }

    fn view_offset(&self) -> usize {
        // Compute the visual row offset for the cursor. Visual layout has top three rows
        // reserved (Add line, Global mode, and blank), then each block consumes 1 (title) + N value rows
        // where N = ceil(length/registers_per_row). Using 4 registers per row for 80-column terminals.
        let registers_per_row = 4;
        let mut offset = 0usize;
        // Start with top three rows
        offset += 3;

        // Build items and walk until we find the current selection
        if *self == ModbusDashboardCursor::AddLine {
            return 0;
        }
        if *self == ModbusDashboardCursor::ModbusMode {
            return 1;
        }

        let items_vec = build_modbus_items_vec();
        // Walk items, accumulate heights until we reach the target
        for (idx, item) in items_vec.iter().enumerate() {
            let config_rows = 4usize; // Reduced by 1 since we removed individual ModbusMode
            let reg_rows = (item.register_length as usize)
                .div_ceil(registers_per_row)
                .max(0usize);
            let rows = 1 + config_rows + reg_rows;

            match self {
                ModbusDashboardCursor::StationId { index } if *index == idx => {
                    return offset + 1;
                }
                ModbusDashboardCursor::RegisterMode { index } if *index == idx => {
                    return offset + 2;
                }
                ModbusDashboardCursor::RegisterStartAddress { index } if *index == idx => {
                    return offset + 3;
                }
                ModbusDashboardCursor::RegisterLength { index } if *index == idx => {
                    return offset + 4;
                }
                ModbusDashboardCursor::Register {
                    slave_index,
                    register_index,
                } if *slave_index == idx => {
                    let cell_row = register_index / registers_per_row;
                    return offset + 1 + config_rows + cell_row;
                }
                _ => {
                    offset += rows + 1;
                }
            }
        }

        offset
    }
}

/// Helper to build the per-port items vector in a single consistent place.
fn build_modbus_items_vec() -> Vec<crate::status::types::modbus::ModbusRegisterItem> {
    let mut items_vec: Vec<crate::status::types::modbus::ModbusRegisterItem> = Vec::new();

    let items_opt = crate::tui::status::read_status(|status| {
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            let port_name = status.ports.order.get(*selected_port).cloned();
            Ok(port_name)
        } else {
            Ok(None)
        }
    })
    .ok()
    .flatten();

    if let Some(port_name) = items_opt {
        if let Ok(Some(port_data)) =
            crate::tui::status::read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))
        {
            let crate::status::types::port::PortConfig::Modbus { mode: _, stations } =
                &port_data.config;
            for it in stations.iter() {
                // Just add the item as-is since the global mode is now stored separately
                items_vec.push(it.clone());
            }
        }
    }

    items_vec
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
