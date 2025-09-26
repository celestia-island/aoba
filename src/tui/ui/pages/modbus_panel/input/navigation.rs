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
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;
            
            // Handle horizontal navigation for register table
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register { slave_index, register_index } => {
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
                types::cursor::ModbusDashboardCursor::Register { slave_index, register_index } => {
                    // Get register length to check bounds
                    let max_register = read_status(|status| {
                        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                            let port_name = format!("COM{}", selected_port + 1);
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                if let Ok(port_guard) = port_entry.read() {
                                    if let types::port::PortConfig::Modbus { masters, slaves } = &port_guard.config {
                                        let mut all_items: Vec<_> = masters.iter().chain(slaves.iter()).collect();
                                        if let Some(item) = all_items.get(slave_index) {
                                            return Ok(item.register_length as usize);
                                        }
                                    }
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
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;
            
            // Handle vertical navigation for register table with 8-register row alignment
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register { slave_index, register_index } => {
                    let col = register_index % 8;
                    let row = register_index / 8;
                    
                    if row > 0 {
                        // Move up one row, same column
                        let target_index = (row - 1) * 8 + col;
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: target_index,
                        }
                    } else {
                        // At top row, find nearest valid position in previous item's register table
                        // For now, fall back to standard navigation
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
        KeyCode::Down | KeyCode::Char('j') => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;
            
            // Handle vertical navigation for register table with 8-register row alignment
            let new_cursor = match current_cursor {
                types::cursor::ModbusDashboardCursor::Register { slave_index, register_index } => {
                    let col = register_index % 8;
                    let row = register_index / 8;
                    
                    // Get register length to check bounds
                    let max_register = read_status(|status| {
                        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                            let port_name = format!("COM{}", selected_port + 1);
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                if let Ok(port_guard) = port_entry.read() {
                                    if let types::port::PortConfig::Modbus { masters, slaves } = &port_guard.config {
                                        let mut all_items: Vec<_> = masters.iter().chain(slaves.iter()).collect();
                                        if let Some(item) = all_items.get(slave_index) {
                                            return Ok(item.register_length as usize);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(0)
                    })?;
                    
                    let target_index = (row + 1) * 8 + col;
                    if target_index < max_register {
                        // Move down one row, same column
                        types::cursor::ModbusDashboardCursor::Register {
                            slave_index,
                            register_index: target_index,
                        }
                    } else {
                        // At bottom row or would exceed register count, fall back to standard navigation
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
