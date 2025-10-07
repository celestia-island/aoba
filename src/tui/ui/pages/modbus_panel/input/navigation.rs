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
    log::info!("🟠 ModbusDashboard navigation input: {:?}", key.code);
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
                        // At the first register, jump to RegisterLength
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
                    // Get register length and check if there's a next station
                    let (max_register, has_next_station) = read_status(|status| {
                        let port_name_opt = match &status.page {
                            types::Page::ModbusDashboard { selected_port, .. } => {
                                status.ports.order.get(*selected_port).cloned()
                            }
                            _ => None,
                        };
                        if let Some(port_name) = port_name_opt {
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                if let Ok(port_guard) = port_entry.read() {
                                    let types::port::PortConfig::Modbus { mode: _, stations } =
                                        &port_guard.config;
                                    let all_items: Vec<_> = stations.iter().collect();
                                    if let Some(item) = all_items.get(slave_index) {
                                        let has_next = slave_index + 1 < all_items.len();
                                        return Ok((item.register_length as usize, has_next));
                                    }
                                }
                            }
                        }
                        Ok((0, false))
                    })?;

                    if register_index + 1 < max_register {
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: register_index + 1,
                        }
                    } else if has_next_station {
                        // Jump to next station's first register
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index: slave_index + 1,
                            register_index: 0,
                        }
                    } else {
                        // Stay at current position
                        current_cursor
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

            // Handle vertical navigation for register table with dynamic registers per row
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register {
                    slave_index,
                    register_index,
                } => {
                    // Use 4 registers per row for 80-column terminals
                    let registers_per_row = 4;

                    // Get register length
                    let _max_register = read_status(|status| {
                        let port_name_opt = match &status.page {
                            types::Page::ModbusDashboard { selected_port, .. } => {
                                status.ports.order.get(*selected_port).cloned()
                            }
                            _ => None,
                        };
                        if let Some(port_name) = port_name_opt {
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                if let Ok(port_guard) = port_entry.read() {
                                    let types::port::PortConfig::Modbus { mode: _, stations } =
                                        &port_guard.config;
                                    let all_items: Vec<_> = stations.iter().collect();
                                    if let Some(item) = all_items.get(slave_index) {
                                        return Ok(item.register_length as usize);
                                    }
                                }
                            }
                        }
                        Ok(0)
                    })?;

                    if register_index >= registers_per_row {
                        // Move up by registers_per_row
                        let target_index = register_index.saturating_sub(registers_per_row);
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: target_index,
                        }
                    } else if register_index > 0 {
                        // In first row but not at index 0, jump to first register
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: 0,
                        }
                    } else {
                        // At first register, jump to RegisterLength
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
                    // Use 4 registers per row for 80-column terminals
                    let registers_per_row = 4;

                    // Get register length and check if there's a next station
                    let (max_register, has_next_slave) = read_status(|status| {
                        let port_name_opt = match &status.page {
                            types::Page::ModbusDashboard { selected_port, .. } => {
                                status.ports.order.get(*selected_port).cloned()
                            }
                            _ => None,
                        };
                        if let Some(port_name) = port_name_opt {
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                if let Ok(port_guard) = port_entry.read() {
                                    let types::port::PortConfig::Modbus { mode: _, stations } =
                                        &port_guard.config;
                                    let all_items: Vec<_> = stations.iter().collect();
                                    if let Some(item) = all_items.get(slave_index) {
                                        let has_next = slave_index + 1 < all_items.len();
                                        return Ok((item.register_length as usize, has_next));
                                    }
                                }
                            }
                        }
                        Ok((0, false))
                    })?;

                    let target_index = register_index + registers_per_row;

                    if target_index < max_register {
                        // Normal case: move down by registers_per_row
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: target_index,
                        }
                    } else if register_index < max_register - 1 {
                        // Target goes beyond, but not at last register yet
                        // Jump to last register
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: max_register - 1,
                        }
                    } else {
                        // At last register
                        if has_next_slave {
                            // Jump to next station's first register
                            types::cursor::ModbusDashboardCursor::Register {
                                slave_index: slave_index + 1,
                                register_index: 0,
                            }
                        } else {
                            // Stay at last register
                            current_cursor
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
            log::info!("🟠 Enter key pressed in ModbusDashboard");
            handle_enter_action(bus)?;
            log::info!("🟠 handle_enter_action completed successfully");
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
