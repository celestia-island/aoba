use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use super::actions::{handle_enter_action, handle_leave_page};
use crate::i18n::lang;
use crate::tui::status as types;
use crate::tui::status::cursor::Cursor;
use crate::tui::status::{read_status, write_status};
use crate::tui::utils::bus::{Bus, UiToCore};

pub fn handle_navigation_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    log::info!("🟠 ModbusDashboard navigation input: {:?}", key.code);

    // Check for Ctrl modifier
    let has_ctrl = key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL);

    // Handle Ctrl+S for saving configuration
    if matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')) && has_ctrl {
        log::info!("💾 Ctrl+S pressed - saving configuration and enabling port");
        // This will trigger port enable with current configuration
        handle_save_config(bus)?;
        return Ok(());
    }

    match key.code {
        KeyCode::PageUp => {
            if has_ctrl {
                // Ctrl+PageUp: Jump to first group (AddLine)
                let new_cursor = types::cursor::ModbusDashboardCursor::AddLine;
                let new_offset = new_cursor.view_offset();
                write_status(|status| {
                    if let crate::tui::status::Page::ModbusDashboard {
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
            } else {
                // PageUp: Jump to previous station group
                let current_cursor = read_status(|status| match &status.page {
                    crate::tui::status::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
                    _ => Ok(types::cursor::ModbusDashboardCursor::AddLine),
                })?;

                let new_cursor = jump_to_prev_group(current_cursor)?;
                let new_offset = new_cursor.view_offset();
                write_status(|status| {
                    if let crate::tui::status::Page::ModbusDashboard {
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
            }
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::PageDown => {
            if has_ctrl {
                // Ctrl+PageDown: Jump to last group (first item of last station if exists)
                let new_cursor = jump_to_last_group()?;
                let new_offset = new_cursor.view_offset();
                write_status(|status| {
                    if let crate::tui::status::Page::ModbusDashboard {
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
            } else {
                // PageDown: Jump to next station group
                let current_cursor = read_status(|status| match &status.page {
                    crate::tui::status::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
                    _ => Ok(types::cursor::ModbusDashboardCursor::AddLine),
                })?;

                let new_cursor = jump_to_next_group(current_cursor)?;
                let new_offset = new_cursor.view_offset();
                write_status(|status| {
                    if let crate::tui::status::Page::ModbusDashboard {
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
            }
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let current_cursor = read_status(|status| match &status.page {
                crate::tui::status::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
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
                if let crate::tui::status::Page::ModbusDashboard {
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
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let current_cursor = read_status(|status| {
                if let crate::tui::status::Page::ModbusDashboard { cursor, .. } = &status.page {
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
                            crate::tui::status::Page::ModbusDashboard { selected_port, .. } => {
                                status.ports.order.get(*selected_port).cloned()
                            }
                            _ => None,
                        };
                        if let Some(port_name) = port_name_opt {
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                let port = port_entry;
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port.config;
                                let all_items: Vec<_> = stations.iter().collect();
                                if let Some(item) = all_items.get(slave_index) {
                                    let has_next = slave_index + 1 < all_items.len();
                                    return Ok((item.register_length as usize, has_next));
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
                if let crate::tui::status::Page::ModbusDashboard {
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
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let current_cursor = read_status(|status| match &status.page {
                crate::tui::status::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
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
                            crate::tui::status::Page::ModbusDashboard { selected_port, .. } => {
                                status.ports.order.get(*selected_port).cloned()
                            }
                            _ => None,
                        };
                        if let Some(port_name) = port_name_opt {
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                let port = port_entry;
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port.config;
                                let all_items: Vec<_> = stations.iter().collect();
                                if let Some(item) = all_items.get(slave_index) {
                                    return Ok(item.register_length as usize);
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
                if let crate::tui::status::Page::ModbusDashboard {
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
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let current_cursor = read_status(|status| match &status.page {
                crate::tui::status::Page::ModbusDashboard { cursor, .. } => Ok(*cursor),
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
                            crate::tui::status::Page::ModbusDashboard { selected_port, .. } => {
                                status.ports.order.get(*selected_port).cloned()
                            }
                            _ => None,
                        };
                        if let Some(port_name) = port_name_opt {
                            if let Some(port_entry) = status.ports.map.get(&port_name) {
                                let port = port_entry;
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port.config;
                                let all_items: Vec<_> = stations.iter().collect();
                                if let Some(item) = all_items.get(slave_index) {
                                    let has_next = slave_index + 1 < all_items.len();
                                    return Ok((item.register_length as usize, has_next));
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
                if let crate::tui::status::Page::ModbusDashboard {
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
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
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

/// Handle saving configuration with Ctrl+S
/// This marks the config as saved and triggers port enable if not already enabled
fn handle_save_config(bus: &Bus) -> Result<()> {
    use chrono::Local;

    let selected_port = read_status(|status| {
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    let port_name_opt = read_status(|status| Ok(status.ports.order.get(selected_port).cloned()))?;

    if let Some(port_name) = port_name_opt {
        if let Some(port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
            // Check if port has any stations configured
            let has_stations = match &port.config {
                types::port::PortConfig::Modbus { stations, .. } => !stations.is_empty(),
            };

            if !has_stations {
                // Show error if no stations configured
                write_status(|status| {
                    status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                        message: lang().index.err_modbus_config_empty.clone(),
                        timestamp: Local::now(),
                    });
                    Ok(())
                })?;
                bus.ui_tx
                    .send(UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                return Ok(());
            }

            // Mark config as not modified
            write_status(|status| {
                let port = status
                    .ports
                    .map
                    .get_mut(&port_name)
                    .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                port.config_modified = false;
                // Set status to AppliedSuccess for 3 seconds
                port.status_indicator = types::port::PortStatusIndicator::AppliedSuccess {
                    timestamp: Local::now(),
                };
                Ok(())
            })?;

            // Check if port is already enabled
            let is_enabled = matches!(port.state, types::port::PortState::OccupiedByThis);

            if !is_enabled {
                // Enable the port if not already enabled
                log::info!("Port not enabled, triggering enable via ToggleRuntime");
                bus.ui_tx
                    .send(UiToCore::ToggleRuntime(port_name.clone()))
                    .map_err(|err| anyhow!(err))?;
            } else {
                // Port already enabled, just restart it with new config
                log::info!("Port already enabled, restarting with new config");
                bus.ui_tx
                    .send(UiToCore::ToggleRuntime(port_name.clone()))
                    .map_err(|err| anyhow!(err))?;
                bus.ui_tx
                    .send(UiToCore::ToggleRuntime(port_name))
                    .map_err(|err| anyhow!(err))?;
            }

            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
        }
    }

    Ok(())
}

/// Jump to previous station group
/// - If on AddLine or ModbusMode, stay at AddLine (first group)
/// - If on a station's item, jump to previous station's first item (StationId)
/// - If on first station, jump to ModbusMode
fn jump_to_prev_group(
    current_cursor: types::cursor::ModbusDashboardCursor,
) -> Result<types::cursor::ModbusDashboardCursor> {
    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            // Already at first group
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
        types::cursor::ModbusDashboardCursor::ModbusMode => {
            // Jump to AddLine
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
        types::cursor::ModbusDashboardCursor::StationId { index }
        | types::cursor::ModbusDashboardCursor::RegisterMode { index }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { index }
        | types::cursor::ModbusDashboardCursor::RegisterLength { index }
        | types::cursor::ModbusDashboardCursor::Register {
            slave_index: index, ..
        } => {
            if index == 0 {
                // Jump to ModbusMode (second item in first group)
                Ok(types::cursor::ModbusDashboardCursor::ModbusMode)
            } else {
                // Jump to previous station's first item
                Ok(types::cursor::ModbusDashboardCursor::StationId { index: index - 1 })
            }
        }
    }
}

/// Jump to next station group
/// - If on AddLine, jump to ModbusMode
/// - If on ModbusMode, jump to first station (if exists) or stay at ModbusMode
/// - If on a station's item, jump to next station's first item (if exists) or stay
fn jump_to_next_group(
    current_cursor: types::cursor::ModbusDashboardCursor,
) -> Result<types::cursor::ModbusDashboardCursor> {
    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            // Jump to ModbusMode
            Ok(types::cursor::ModbusDashboardCursor::ModbusMode)
        }
        types::cursor::ModbusDashboardCursor::ModbusMode => {
            // Jump to first station if exists
            let has_stations = read_status(|status| {
                if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } =
                    &status.page
                {
                    if let Some(port_name) = status.ports.order.get(*selected_port) {
                        if let Some(port_entry) = status.ports.map.get(port_name) {
                            let port = port_entry;
                            let types::port::PortConfig::Modbus { mode: _, stations } =
                                &port.config;
                            return Ok(!stations.is_empty());
                        }
                    }
                }
                Ok(false)
            })?;

            if has_stations {
                Ok(types::cursor::ModbusDashboardCursor::StationId { index: 0 })
            } else {
                // No stations, stay at ModbusMode
                Ok(types::cursor::ModbusDashboardCursor::ModbusMode)
            }
        }
        types::cursor::ModbusDashboardCursor::StationId { index }
        | types::cursor::ModbusDashboardCursor::RegisterMode { index }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { index }
        | types::cursor::ModbusDashboardCursor::RegisterLength { index }
        | types::cursor::ModbusDashboardCursor::Register {
            slave_index: index, ..
        } => {
            // Check if next station exists
            let has_next = read_status(|status| {
                if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } =
                    &status.page
                {
                    if let Some(port_name) = status.ports.order.get(*selected_port) {
                        if let Some(port_entry) = status.ports.map.get(port_name) {
                            let port = port_entry;
                            let types::port::PortConfig::Modbus { mode: _, stations } =
                                &port.config;
                            let all_items: Vec<_> = stations.iter().collect();
                            return Ok(index + 1 < all_items.len());
                        }
                    }
                }
                Ok(false)
            })?;

            if has_next {
                Ok(types::cursor::ModbusDashboardCursor::StationId { index: index + 1 })
            } else {
                // Stay at current station's first item
                Ok(types::cursor::ModbusDashboardCursor::StationId { index })
            }
        }
    }
}

/// Jump to last station group (first item of last station if exists, otherwise ModbusMode)
fn jump_to_last_group() -> Result<types::cursor::ModbusDashboardCursor> {
    let last_station_index = read_status(|status| {
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port_entry) = status.ports.map.get(port_name) {
                    let port = port_entry;
                    let types::port::PortConfig::Modbus { mode: _, stations } = &port.config;
                    let all_items: Vec<_> = stations.iter().collect();
                    if !all_items.is_empty() {
                        return Ok(Some(all_items.len() - 1));
                    }
                }
            }
        }
        Ok(None)
    })?;

    if let Some(index) = last_station_index {
        Ok(types::cursor::ModbusDashboardCursor::StationId { index })
    } else {
        // No stations, stay at ModbusMode
        Ok(types::cursor::ModbusDashboardCursor::ModbusMode)
    }
}
