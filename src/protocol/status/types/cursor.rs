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
///
/// This cursor carries explicit identity information for the selected element so
/// renderers and input handlers can determine exactly which block and which
/// register (cell) is active without relying on fragile numeric row-to-block
/// conversions. To keep the enum serializable, `mode` is stored as `u8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusDashboardCursor {
    AddLine,

    ModbusMode {
        index: usize,
    },
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
        // Helper: build a flat ordered list of cursor positions based on current items
        let mut flat: Vec<ModbusDashboardCursor> = Vec::new();
        // Add top AddLine
        flat.push(ModbusDashboardCursor::AddLine);

        // Try to collect current items (masters + slaves or placeholder) similar to renderer
        let items_opt = read_status(|status| {
            if let crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. } =
                &status.page
            {
                let port_name = status.ports.order.get(*selected_port).cloned();
                Ok(port_name)
            } else {
                Ok(None)
            }
        })
        .ok()
        .flatten();

        if let Some(port_name) = items_opt {
            if let Ok(port_entry_opt) =
                read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))
            {
                if let Some(port_entry) = port_entry_opt {
                    if let Ok(port_data) = port_entry.read() {
                        let crate::protocol::status::types::port::PortConfig::Modbus {
                            masters,
                            slaves,
                        } = &port_data.config;
                        let mut items_vec: Vec<
                            crate::protocol::status::types::modbus::ModbusRegisterItem,
                        > = Vec::new();
                        for it in masters.iter() {
                            items_vec.push(it.clone());
                        }
                        // Only include actual configured masters and slaves.
                        // When there are no items, leave `items_vec` empty so the
                        // flat cursor list will only contain `AddLine` and cursor
                        // movement will not advance into non-existent entries.
                        for it in slaves.iter() {
                            items_vec.push(it.clone());
                        }

                        for (idx, item) in items_vec.iter().enumerate() {
                            // per-item editable fields
                            flat.push(ModbusDashboardCursor::ModbusMode { index: idx });
                            flat.push(ModbusDashboardCursor::StationId { index: idx });
                            flat.push(ModbusDashboardCursor::RegisterMode { index: idx });
                            flat.push(ModbusDashboardCursor::RegisterStartAddress { index: idx });
                            flat.push(ModbusDashboardCursor::RegisterLength { index: idx });
                            // per-register entries
                            let regs = item.register_length as usize;
                            for reg in 0..regs {
                                flat.push(ModbusDashboardCursor::Register {
                                    slave_index: idx,
                                    register_index: reg,
                                });
                            }
                        }
                    }
                }
            }
        }

        // find current position in flat list
        let cur_pos = flat.iter().position(|c| *c == self).unwrap_or(0);
        if cur_pos == 0 {
            // keep at AddLine
            flat[0]
        } else {
            flat[cur_pos - 1]
        }
    }

    fn next(self) -> Self {
        let mut flat: Vec<ModbusDashboardCursor> = Vec::new();
        flat.push(ModbusDashboardCursor::AddLine);

        let items_opt = read_status(|status| {
            if let crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. } =
                &status.page
            {
                let port_name = status.ports.order.get(*selected_port).cloned();
                Ok(port_name)
            } else {
                Ok(None)
            }
        })
        .ok()
        .flatten();

        if let Some(port_name) = items_opt {
            if let Ok(port_entry_opt) =
                read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))
            {
                if let Some(port_entry) = port_entry_opt {
                    if let Ok(port_data) = port_entry.read() {
                        let crate::protocol::status::types::port::PortConfig::Modbus {
                            masters,
                            slaves,
                        } = &port_data.config;
                        let mut items_vec: Vec<
                            crate::protocol::status::types::modbus::ModbusRegisterItem,
                        > = Vec::new();
                        for it in masters.iter() {
                            items_vec.push(it.clone());
                        }
                        // Only include actual configured masters and slaves.
                        // When there are no items, leave `items_vec` empty so the
                        // flat cursor list will only contain `AddLine` and cursor
                        // movement will not advance into non-existent entries.
                        for it in slaves.iter() {
                            items_vec.push(it.clone());
                        }

                        for (idx, item) in items_vec.iter().enumerate() {
                            flat.push(ModbusDashboardCursor::ModbusMode { index: idx });
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
                    }
                }
            }
        }

        let cur_pos = flat.iter().position(|c| *c == self).unwrap_or(0);
        if cur_pos + 1 >= flat.len() {
            // stay at last element (do not wrap)
            flat[flat.len() - 1]
        } else {
            flat[cur_pos + 1]
        }
    }

    fn view_offset(&self) -> usize {
        // Compute the visual row offset for the cursor. Visual layout has top two rows
        // reserved (Add line and blank), then each block consumes 1 (title) + N value rows
        // where N = ceil(length/8).
        let mut offset = 0usize;
        // Start with top two rows
        offset += 2;

        // Build items and walk until we find the current selection
        let items_opt = read_status(|status| {
            if let crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. } =
                &status.page
            {
                let port_name = status.ports.order.get(*selected_port).cloned();
                Ok(port_name)
            } else {
                Ok(None)
            }
        })
        .ok()
        .flatten();

        if *self == ModbusDashboardCursor::AddLine {
            return 0;
        }

        if let Some(port_name) = items_opt {
            if let Ok(port_entry_opt) =
                read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))
            {
                if let Some(port_entry) = port_entry_opt {
                    if let Ok(port_data) = port_entry.read() {
                        let crate::protocol::status::types::port::PortConfig::Modbus {
                            masters,
                            slaves,
                        } = &port_data.config;
                        let mut items_vec: Vec<
                            crate::protocol::status::types::modbus::ModbusRegisterItem,
                        > = Vec::new();
                        for it in masters.iter() {
                            items_vec.push(it.clone());
                        }
                        if slaves.is_empty() {
                            let default_item = crate::protocol::status::types::modbus::ModbusRegisterItem { connection_mode: crate::protocol::status::types::modbus::ModbusConnectionMode::Slave, station_id: 1, register_mode: crate::protocol::status::types::modbus::RegisterMode::Coils, register_address: 0, register_length: 8, req_success: 0, req_total: 0, next_poll_at: std::time::Instant::now(), pending_requests: Vec::new(), values: Vec::new() };
                            items_vec.push(default_item);
                        } else {
                            for it in slaves.iter() {
                                items_vec.push(it.clone());
                            }
                        }

                        // Walk items, accumulate heights until we reach the target
                        for (idx, item) in items_vec.iter().enumerate() {
                            // compute block height in rows
                            let rows = 1 + ((item.register_length as usize + 7) / 8);

                            // If cursor refers to this block (by index), return appropriate offset.
                            match self {
                                ModbusDashboardCursor::ModbusMode { index }
                                | ModbusDashboardCursor::StationId { index }
                                | ModbusDashboardCursor::RegisterMode { index }
                                | ModbusDashboardCursor::RegisterStartAddress { index }
                                | ModbusDashboardCursor::RegisterLength { index }
                                    if *index == idx =>
                                {
                                    return offset; // point to block title row
                                }
                                ModbusDashboardCursor::Register {
                                    slave_index,
                                    register_index,
                                } if *slave_index == idx => {
                                    let cell_row = register_index / 8;
                                    return offset + 1 + cell_row;
                                }
                                _ => {
                                    // advance by block height + separator
                                    offset += rows + 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // fallback
        offset
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
