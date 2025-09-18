use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, write_status},
    tui::utils::bus::Bus,
};

/// Handle input for config panel. Sends commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Create a snapshot of the current status (previously passed as `app`)
    let snapshot = read_status(|s| Ok(s.clone()))?;
    // Derive selected row in panel (same logic as render_kv_panel)
    let selected_row = super::components::derive_selection(&snapshot);

    // Determine number of fields shown in panel
    // New structure: 2 fields in group 1 + 4 fields in group 2 + 1 empty line = 7 total
    let _field_count = 7usize; // Enable, Protocol, (empty), Baud, DataBits, Parity, StopBits
    let navigable_fields = vec![0, 1, 3, 4, 5, 6]; // Skip the empty line at index 2

    // If currently in edit mode for this port, handle edit-specific keys
    // New location: per-page fields are stored inside Page::ModbusConfig
    let mut in_edit = false;
    if let types::Page::ModbusConfig {
        edit_active: config_edit_active,
        ..
    } = &snapshot.page
    {
        in_edit = *config_edit_active;
    }

    if in_edit {
        // We are editing a field: handle text input and control keys
        match key.code {
            KeyCode::Char(c) => {
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        let pos = (*config_edit_cursor_pos).min(config_edit_buffer.len());
                        config_edit_buffer.insert(pos, c);
                        *config_edit_cursor_pos = pos + 1;
                    }
                    Ok(())
                })?;
                Ok(())
            }
            KeyCode::Backspace => {
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        let pos = *config_edit_cursor_pos;
                        if pos > 0 && pos <= config_edit_buffer.len() {
                            config_edit_buffer.remove(pos - 1);
                            *config_edit_cursor_pos = pos - 1;
                        }
                    }
                    Ok(())
                })?;
                Ok(())
            }
            KeyCode::Left => {
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        if *config_edit_cursor_pos > 0 {
                            *config_edit_cursor_pos -= 1;
                        }
                    }
                    Ok(())
                })?;
                Ok(())
            }
            KeyCode::Right => {
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        let len = config_edit_buffer.len();
                        if *config_edit_cursor_pos < len {
                            *config_edit_cursor_pos += 1;
                        }
                    }
                    Ok(())
                })?;
                Ok(())
            }
            KeyCode::Enter => {
                // Commit edit: write buffer back to PortData field
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        selected_port,
                        edit_active: config_edit_active,
                        edit_cursor: config_edit_cursor,
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        if let Some(port_name) = s.ports.order.get(*selected_port) {
                            if let Some(pd) = s.ports.map.get_mut(port_name) {
                                let val = config_edit_buffer.clone();
                                match *config_edit_cursor {
                                    3 => {
                                        // Baud rate
                                        if let Ok(v) = val.parse::<u32>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.baud = v;
                                            }
                                        }
                                    }
                                    4 => {
                                        // Data bits
                                        if let Ok(v) = val.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.data_bits = v;
                                            }
                                        }
                                    }
                                    5 => {
                                        // Parity
                                        if let Some(rt) = pd.runtime.as_mut() {
                                            rt.current_cfg.parity = match val.as_str() {
                                                "None" | "none" => serialport::Parity::None,
                                                "Odd" | "odd" => serialport::Parity::Odd,
                                                "Even" | "even" => serialport::Parity::Even,
                                                _ => rt.current_cfg.parity,
                                            }
                                        }
                                    }
                                    6 => {
                                        // Stop bits
                                        if let Ok(v) = val.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.stop_bits = v;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        // Exit edit mode and clear buffer
                        *config_edit_active = false;
                        config_edit_buffer.clear();
                        *config_edit_cursor_pos = 0;
                    }
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Esc => {
                // Cancel edit
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        edit_active: config_edit_active,
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        *config_edit_active = false;
                        config_edit_buffer.clear();
                        *config_edit_cursor_pos = 0;
                    }
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            _ => Ok(()),
        }
    } else {
        // Not in edit mode: handle navigation and enter to toggle/select/edit
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
                // Navigate between fields within the config panel
                handle_field_navigation(key.code, &navigable_fields)?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Enter => {
                // Handle different actions based on selected field
                handle_enter_key(&snapshot, selected_row, bus)
            }
            KeyCode::Esc => {
                // Return to the main entry page
                let cursor = if let types::Page::ModbusConfig { selected_port, .. } = &snapshot.page
                {
                    Some(types::ui::EntryCursor::Com {
                        idx: *selected_port,
                    })
                } else {
                    None
                };
                write_status(|s| {
                    s.page = types::Page::Entry { cursor };
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Handle navigation between fields in the config panel
fn handle_field_navigation(key_code: KeyCode, navigable_fields: &[usize]) -> Result<()> {
    write_status(|s| {
        if let types::Page::ModbusConfig { edit_cursor, .. } = &mut s.page {
            let current_pos = navigable_fields
                .iter()
                .position(|&field| field == *edit_cursor)
                .unwrap_or(0);
            
            match key_code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if current_pos > 0 {
                        *edit_cursor = navigable_fields[current_pos - 1];
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if current_pos < navigable_fields.len() - 1 {
                        *edit_cursor = navigable_fields[current_pos + 1];
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })
}

/// Handle Enter key press based on the selected field
fn handle_enter_key(snapshot: &types::Status, selected_row: usize, bus: &Bus) -> Result<()> {
    match selected_row {
        0 => {
            // Enable Port toggle - attempt to toggle port state
            handle_port_toggle(snapshot, bus)
        }
        1 => {
            // Protocol selection - navigate to the appropriate config page
            handle_protocol_navigation(snapshot, bus)
        }
        3..=6 => {
            // Serial parameter fields - enter edit mode
            handle_parameter_edit(snapshot, selected_row, bus)
        }
        _ => Ok(()),
    }
}

/// Handle port enable/disable toggle with error checking
fn handle_port_toggle(snapshot: &types::Status, bus: &Bus) -> Result<()> {
    if let types::Page::ModbusConfig { selected_port, .. } = &snapshot.page {
        if let Some(port_name) = snapshot.ports.order.get(*selected_port) {
            if let Some(pd) = snapshot.ports.map.get(port_name) {
                match pd.state {
                    crate::protocol::status::types::port::PortState::Free => {
                        // Port is free, try to enable it
                        // TODO: Implement actual port opening logic
                        // For now, just update the state (this should be moved to core logic)
                        write_status(|s| {
                            if let Some(port_data) = s.ports.map.get_mut(port_name) {
                                port_data.state = crate::protocol::status::types::port::PortState::OccupiedByThis;
                            }
                            Ok(())
                        })?;
                    }
                    crate::protocol::status::types::port::PortState::OccupiedByThis => {
                        // Port is occupied by us, disable it
                        write_status(|s| {
                            if let Some(port_data) = s.ports.map.get_mut(port_name) {
                                port_data.state = crate::protocol::status::types::port::PortState::Free;
                            }
                            Ok(())
                        })?;
                    }
                    crate::protocol::status::types::port::PortState::OccupiedByOther => {
                        // Port is occupied by another process - show error
                        write_status(|s| {
                            s.temporarily.error = Some(types::ErrorInfo {
                                message: "Port is occupied by another process".to_string(),
                                timestamp: chrono::Local::now(),
                            });
                            Ok(())
                        })?;
                    }
                }
            }
        }
    }
    
    bus.ui_tx
        .send(crate::tui::utils::bus::UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

/// Handle protocol selection navigation
fn handle_protocol_navigation(snapshot: &types::Status, bus: &Bus) -> Result<()> {
    // For now, always navigate to modbus panel as specified in requirements
    // Later this can be extended to support MQTT and other protocols
    if let types::Page::ModbusConfig { selected_port, .. } = &snapshot.page {
        write_status(|s| {
            s.page = types::Page::ModbusDashboard {
                selected_port: *selected_port,
                cursor: 0,
                editing_field: None,
                input_buffer: String::new(),
                edit_choice_index: None,
                edit_confirmed: false,
                master_cursor: 0,
                master_field_selected: false,
                master_field_editing: false,
                master_edit_field: None,
                master_edit_index: None,
                master_input_buffer: String::new(),
                poll_round_index: 0,
                in_flight_reg_index: None,
            };
            Ok(())
        })?;
        
        bus.ui_tx
            .send(crate::tui::utils::bus::UiToCore::Refresh)
            .map_err(|err| anyhow!(err))?;
    }
    
    Ok(())
}

/// Handle entering edit mode for serial parameters
fn handle_parameter_edit(snapshot: &types::Status, selected_row: usize, bus: &Bus) -> Result<()> {
    if let types::Page::ModbusConfig { selected_port, .. } = &snapshot.page {
        if let Some(port_name) = snapshot.ports.order.get(*selected_port) {
            if let Some(pd) = snapshot.ports.map.get(port_name) {
                // Get initial value for the field being edited
                let init_buf = get_field_initial_value(pd, selected_row);
                
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        edit_active,
                        edit_cursor,
                        edit_buffer,
                        edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        *edit_active = true;
                        *edit_cursor = selected_row;
                        *edit_buffer = init_buf.clone();
                        *edit_cursor_pos = init_buf.len();
                    }
                    Ok(())
                })?;
                
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
            }
        }
    }
    Ok(())
}

/// Get initial value for editing a specific field
fn get_field_initial_value(
    pd: &crate::protocol::status::types::port::PortData,
    field_index: usize,
) -> String {
    match field_index {
        3 => {
            // Baud rate
            pd.runtime
                .as_ref()
                .map(|rt| rt.current_cfg.baud.to_string())
                .unwrap_or_else(|| "9600".to_string())
        }
        4 => {
            // Data bits
            pd.runtime
                .as_ref()
                .map(|rt| rt.current_cfg.data_bits.to_string())
                .unwrap_or_else(|| "8".to_string())
        }
        5 => {
            // Parity
            pd.runtime
                .as_ref()
                .map(|rt| format!("{:?}", rt.current_cfg.parity))
                .unwrap_or_else(|| lang().protocol.common.parity_none.clone())
        }
        6 => {
            // Stop bits
            pd.runtime
                .as_ref()
                .map(|rt| rt.current_cfg.stop_bits.to_string())
                .unwrap_or_else(|| "1".to_string())
        }
        _ => String::new(),
    }
}
