use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{self, cursor::Cursor},
        write_status,
    },
    tui::utils::bus::{Bus, UiToCore},
};

use super::actions::{handle_enter_action, handle_leave_page};

pub fn handle_navigation_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => {
            let current_cursor = read_status(|status| match &status.page {
                types::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
                _ => Ok(types::cursor::ModbusDashboardCursor::AddLine),
            })?;

            // Handle horizontal navigation for register table
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register {
                    slave_index,
                    register_index,
                } => {
                    if register_index > 0 {
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: register_index - 1,
                        }
                    } else {
                        current_cursor.prev()
                    }
                }
                _ => current_cursor.prev(),
            };

            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            // Handle horizontal navigation for register table
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register {
                    slave_index,
                    register_index,
                } => {
                    // Get register length to check bounds
                    let max_register = read_status(|status| {
                        let port_name = match &status.page {
                            types::Page::ModbusDashboard { selected_port, .. } => {
                                format!("COM{}", selected_port + 1)
                            }
                            _ => String::new(),
                        };
                        if let Some(port_entry) = status.ports.map.get(&port_name) {
                            if let Ok(port_guard) = port_entry.read() {
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port_guard.config;
                                let all_items: Vec<_> =
                                    stations.iter().collect();
                                if let Some(item) = all_items.get(slave_index) {
                                    return Ok(item.register_length as usize);
                                }
                            }
                        }
                        Ok(0)
                    })?;

                    if register_index + 1 < max_register {
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: register_index + 1,
                        }
                    } else {
                        current_cursor.next()
                    }
                }
                _ => current_cursor.next(),
            };

            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let current_cursor = read_status(|status| match &status.page {
                types::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
                _ => Ok(types::cursor::ModbusDashboardCursor::AddLine),
            })?;

            // Handle vertical navigation for register table with 8-register row alignment
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register {
                    slave_index,
                    register_index,
                } => {
                    let col = register_index % 8;
                    let row = register_index / 8;

                    // Get register info
                    let (max_register, register_start) = read_status(|status| {
                        let port_name = match &status.page {
                            types::Page::ModbusDashboard { selected_port, .. } => {
                                format!("COM{}", selected_port + 1)
                            }
                            _ => String::new(),
                        };
                        if let Some(port_entry) = status.ports.map.get(&port_name) {
                            if let Ok(port_guard) = port_entry.read() {
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port_guard.config;
                                let all_items: Vec<_> =
                                    stations.iter().collect();
                                if let Some(item) = all_items.get(slave_index) {
                                    return Ok((
                                        item.register_length as usize,
                                        item.register_address,
                                    ));
                                }
                            }
                        }
                        Ok((0, 0))
                    })?;

                    if row > 0 {
                        // Try to move up one row, same column
                        let target_index = (row - 1) * 8 + col;

                        // Check if the target position has a valid register
                        let target_address = register_start + target_index as u16;
                        let first_valid_address = register_start;
                        let last_valid_address = register_start + max_register as u16 - 1;

                        if target_address >= first_valid_address
                            && target_address <= last_valid_address
                        {
                            // Valid register at target position
                            types::cursor::ModbusDashboardCursor::Register {
                                slave_index,
                                register_index: target_index,
                            }
                        } else {
                            // Edge case: target position is invalid (would be underscore)
                            // Jump to first valid register in that row (leftmost valid position)
                            let row_start_index = (row - 1) * 8;
                            let first_valid_in_row = if row_start_index < max_register {
                                row_start_index
                            } else {
                                0 // fallback to first register
                            };
                            types::cursor::ModbusDashboardCursor::Register {
                                slave_index,
                                register_index: first_valid_in_row,
                            }
                        }
                    } else {
                        // At top row, jump to register length field (previous config item)
                        types::cursor::ModbusDashboardCursor::RegisterLength { index: slave_index }
                    }
                }
                _ => current_cursor.prev(),
            };

            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let current_cursor = read_status(|status| match &status.page {
                types::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
                _ => Ok(types::cursor::ModbusDashboardCursor::AddLine),
            })?;

            // Handle vertical navigation for register table with 8-register row alignment
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register {
                    slave_index,
                    register_index,
                } => {
                    let col = register_index % 8;
                    let row = register_index / 8;

                    // Get register info and next slave info
                    let (max_register, _register_start, has_next_slave) = read_status(|status| {
                        let port_name = match &status.page {
                            types::Page::ModbusDashboard { selected_port, .. } => {
                                format!("COM{}", selected_port + 1)
                            }
                            _ => String::new(),
                        };
                        if let Some(port_entry) = status.ports.map.get(&port_name) {
                            if let Ok(port_guard) = port_entry.read() {
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port_guard.config;
                                let all_items: Vec<_> =
                                    stations.iter().collect();
                                if let Some(item) = all_items.get(slave_index) {
                                    let has_next = slave_index + 1 < all_items.len();
                                    return Ok((
                                        item.register_length as usize,
                                        item.register_address,
                                        has_next,
                                    ));
                                }
                            }
                        }
                        Ok((0, 0, false))
                    })?;

                    let max_row = (max_register as usize).div_ceil(8);
                    let target_index = (row + 1) * 8 + col;

                    if target_index < max_register {
                        // Normal case: move down one row, same column
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: target_index,
                        }
                    } else if row + 1 < max_row {
                        // Edge case: would move to invalid position in next row
                        // Jump to rightmost valid register (register_length - 1)
                        let last_valid_index = max_register - 1;
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: last_valid_index,
                        }
                    } else {
                        // At bottom row of current slave
                        if has_next_slave {
                            // Jump to connection mode of next slave
                            types::cursor::ModbusDashboardCursor::ModbusMode {
                                index: slave_index + 1,
                            }
                        } else {
                            // Jump to last valid register in current slave
                            let last_valid_index = if max_register > 0 {
                                max_register - 1
                            } else {
                                0
                            };
                            types::cursor::ModbusDashboardCursor::Register {
                                slave_index,
                                register_index: last_valid_index,
                            }
                        }
                    }
                }
                _ => current_cursor.next(),
            };

            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Enter => {
            handle_enter_action(bus)?;
            Ok(())
        }
        KeyCode::Delete => {
            // Handle delete key functionality here
            Ok(())
        }
        KeyCode::Esc => {
            handle_leave_page(bus)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
