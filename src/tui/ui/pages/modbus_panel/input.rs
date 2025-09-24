use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{read_status, types, write_status, with_port_write},
    tui::{ui::components::input_span_handler, utils::bus::Bus},
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Check if we are currently in editing mode
    let editing = read_status(|status| {
        Ok(!matches!(
            status.temporarily.input_raw_buffer,
            types::ui::InputRawBuffer::None
        ))
    })?;

    if editing {
        // Handle input editing
        handle_editing_input(key, bus)
    } else {
        // Handle navigation and normal operations
        handle_navigation_input(key, bus)
    }
}

fn handle_navigation_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    use crate::protocol::status::types::cursor::Cursor;
    use crate::tui::utils::bus::UiToCore;

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            // Compute new cursor outside of the global write lock to avoid
            // calling `read_status` while holding the write lock (which can
            // deadlock because Cursor::prev/next access global status).
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            let new_cursor = current_cursor.prev();
            let new_offset = new_cursor.view_offset();

            // Now write the computed cursor and offset under the write lock.
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

            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;

            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Compute new cursor outside of the global write lock to avoid
            // calling `read_status` while holding the write lock (see comment
            // in Up handling).
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            let new_cursor = current_cursor.next();
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

            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;

            Ok(())
        }
        KeyCode::Enter => {
            // Enter editing mode or execute action
            handle_enter_action(bus)?;
            Ok(())
        }
        KeyCode::Esc => {
            handle_leave_page(bus)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_editing_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Get current cursor to determine editing context
    let current_cursor = read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            Ok(*cursor)
        } else {
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })?;

    match current_cursor {
        types::cursor::ModbusDashboardCursor::ModbusMode { .. }
        | types::cursor::ModbusDashboardCursor::RegisterMode { .. } => {
            // Handle selector editing (use Left/Right to change options)
            let choices = match current_cursor {
                types::cursor::ModbusDashboardCursor::ModbusMode { .. } => Some(2), // Master/Slave
                types::cursor::ModbusDashboardCursor::RegisterMode { .. } => Some(4), // 4 register types
                _ => None,
            };

            input_span_handler::handle_input_span(
                key,
                bus,
                choices,
                move |committed_value: Option<String>| {
                    if let Some(_) = committed_value {
                        // For selector editing, we commit the index value
                        commit_selector_edit(current_cursor)?;
                    }
                    Ok(())
                },
            )
        }
        types::cursor::ModbusDashboardCursor::StationId { .. }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { .. }
        | types::cursor::ModbusDashboardCursor::RegisterLength { .. }
        | types::cursor::ModbusDashboardCursor::Register { .. } => {
            // Handle text input editing
            input_span_handler::handle_input_span(
                key,
                bus,
                None, // No wrapping for text inputs
                move |committed_value: Option<String>| {
                    if let Some(value) = committed_value {
                        commit_text_edit(current_cursor, value)?;
                    }
                    Ok(())
                },
            )
        }
        _ => Ok(()),
    }
}

fn handle_enter_action(bus: &Bus) -> Result<()> {
    use crate::tui::utils::bus::UiToCore;

    let current_cursor = read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            Ok(*cursor)
        } else {
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })?;

    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            // Create new master/slave entry
            create_new_modbus_entry()?;
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
        }
        types::cursor::ModbusDashboardCursor::ModbusMode { .. }
        | types::cursor::ModbusDashboardCursor::RegisterMode { .. } => {
            // Start selector editing
            start_selector_editing(current_cursor)?;
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
        }
        types::cursor::ModbusDashboardCursor::StationId { .. }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { .. }
        | types::cursor::ModbusDashboardCursor::RegisterLength { .. }
        | types::cursor::ModbusDashboardCursor::Register { .. } => {
            // Start text input editing
            start_text_editing(current_cursor)?;
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
        }
    }

    Ok(())
}

fn start_selector_editing(cursor: types::cursor::ModbusDashboardCursor) -> Result<()> {
    write_status(|status| {
        // Set the input buffer to Index mode with current selection
        let current_index = match cursor {
            types::cursor::ModbusDashboardCursor::ModbusMode { .. } => 0, // Default to Master
            types::cursor::ModbusDashboardCursor::RegisterMode { .. } => 2, // Default to Holding
            _ => 0,
        };
        status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::Index(current_index);
        Ok(())
    })
}

fn start_text_editing(cursor: types::cursor::ModbusDashboardCursor) -> Result<()> {
    // Get current value and start string editing
    let current_value = get_current_value_for_cursor(cursor)?;

    write_status(|status| {
        status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::String {
            bytes: current_value.clone().into_bytes(),
            offset: 0,
        };
        Ok(())
    })
}

fn get_current_value_for_cursor(cursor: types::cursor::ModbusDashboardCursor) -> Result<String> {
    // Get current port data and extract the value for this cursor
    let port_data = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status
                .ports
                .map
                .get(&format!("COM{}", selected_port + 1))
                .cloned())
        } else {
            Ok(None)
        }
    })?;

    if let Some(port_entry) = port_data {
        if let Ok(port_guard) = port_entry.read() {
            // PortConfig is expected to be Modbus here; destructure directly
            let types::port::PortConfig::Modbus { masters, slaves } = &port_guard.config;

            match cursor {
                types::cursor::ModbusDashboardCursor::StationId { index } => {
                    let item = masters
                        .get(index)
                        .or_else(|| slaves.get(index - masters.len()));
                    return Ok(item
                        .map(|i| i.station_id.to_string())
                        .unwrap_or_else(|| "1".to_string()));
                }
                types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
                    let item = masters
                        .get(index)
                        .or_else(|| slaves.get(index - masters.len()));
                    return Ok(item
                        .map(|i| i.register_address.to_string())
                        .unwrap_or_else(|| "0".to_string()));
                }
                types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
                    let item = masters
                        .get(index)
                        .or_else(|| slaves.get(index - masters.len()));
                    return Ok(item
                        .map(|i| i.register_length.to_string())
                        .unwrap_or_else(|| "1".to_string()));
                }
                types::cursor::ModbusDashboardCursor::Register {
                    slave_index,
                    register_index,
                } => {
                    let item = masters
                        .get(slave_index)
                        .or_else(|| slaves.get(slave_index - masters.len()));
                    if let Some(item) = item {
                        let value = item.values.get(register_index).copied().unwrap_or(0);
                        return Ok(value.to_string());
                    }
                }
                _ => {}
            }
        }
    }

    Ok("0".to_string())
}

fn commit_selector_edit(_cursor: types::cursor::ModbusDashboardCursor) -> Result<()> {
    let _selected_index = read_status(|status| {
        if let types::ui::InputRawBuffer::Index(i) = status.temporarily.input_raw_buffer {
            Ok(i)
        } else {
            Ok(0)
        }
    })?;

    // Clear the input buffer
    write_status(|status| {
        status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
        Ok(())
    })?;

    // TODO: Update the actual modbus configuration based on cursor and selected_index
    // This will need to modify the port configuration data structure
    Ok(())
}

fn commit_text_edit(_cursor: types::cursor::ModbusDashboardCursor, _value: String) -> Result<()> {
    // Clear the input buffer
    write_status(|status| {
        status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
        Ok(())
    })?;

    // TODO: Update the actual value in the modbus configuration
    // This will need to parse the string value and update the appropriate field
    Ok(())
}

fn create_new_modbus_entry() -> Result<()> {
    use crate::protocol::status::types::modbus::{ModbusRegisterItem, ModbusConnectionMode, RegisterMode};

    // Determine selected port and append a default slave item to its config
    let port_name_opt = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status.ports.order.get(*selected_port).cloned())
        } else {
            Ok(None)
        }
    })?;

    if let Some(port_name) = port_name_opt {
        // Write into status: add a default ModbusRegisterItem to slaves
        write_status(|status| {
            if let Some(port) = status.ports.map.get(&port_name) {
                // Use with_port_write to mutate the PortData in-place
                if with_port_write(port, |port| {
                    if let types::port::PortConfig::Modbus { masters: _, slaves } = &mut port.config {
                        let item = ModbusRegisterItem {
                            connection_mode: ModbusConnectionMode::Slave,
                            station_id: 1,
                            register_mode: RegisterMode::Coils,
                            register_address: 0,
                            register_length: 8,
                            req_success: 0,
                            req_total: 0,
                            next_poll_at: std::time::Instant::now(),
                            pending_requests: Vec::new(),
                            values: Vec::new(),
                        };
                        slaves.push(item);
                        return Some(());
                    }
                    None
                })
                .is_none()
                {
                    log::warn!("create_new_modbus_entry: failed to acquire write lock for {port_name}");
                }
            }
            Ok(())
        })?;
    }

    Ok(())
}

fn handle_leave_page(bus: &Bus) -> Result<()> {
    use crate::tui::utils::bus::UiToCore;

    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    write_status(|status| {
        // Go back to config panel instead of entry page
        status.page = types::Page::ConfigPanel {
            selected_port,
            view_offset: 0,
            cursor: types::cursor::ConfigPanelCursor::EnablePort,
        };
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}
