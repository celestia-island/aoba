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
    // Derive selected cursor in panel
    let selected_cursor = super::components::derive_selection(&snapshot);

    // If currently in edit mode for this port, handle edit-specific keys
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
                        if !config_edit_buffer.is_empty() && *config_edit_cursor_pos > 0 {
                            let pos = (*config_edit_cursor_pos - 1).min(config_edit_buffer.len());
                            config_edit_buffer.remove(pos);
                            *config_edit_cursor_pos = pos;
                        }
                    }
                    Ok(())
                })?;
                Ok(())
            }
            KeyCode::Delete => {
                write_status(|s| {
                    if let types::Page::ModbusConfig {
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        if *config_edit_cursor_pos < config_edit_buffer.len() {
                            config_edit_buffer.remove(*config_edit_cursor_pos);
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
                                    types::ui::ConfigPanelCursor::BaudRate => {
                                        if let Ok(v) = val.parse::<u32>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.baud = v;
                                            }
                                        }
                                    }
                                    types::ui::ConfigPanelCursor::DataBits => {
                                        if let Ok(v) = val.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.data_bits = v;
                                            }
                                        }
                                    }
                                    types::ui::ConfigPanelCursor::Parity => {
                                        if let Some(rt) = pd.runtime.as_mut() {
                                            rt.current_cfg.parity = match val.as_str() {
                                                "None" | "none" => serialport::Parity::None,
                                                "Odd" | "odd" => serialport::Parity::Odd,
                                                "Even" | "even" => serialport::Parity::Even,
                                                _ => rt.current_cfg.parity,
                                            }
                                        }
                                    }
                                    types::ui::ConfigPanelCursor::StopBits => {
                                        if let Ok(v) = val.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.stop_bits = v;
                                            }
                                        }
                                    }
                                    _ => {
                                        // Other cursor types don't support editing
                                    }
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
                // Navigate between fields using cursor system
                write_status(|s| {
                    if let types::Page::ModbusConfig { edit_cursor, .. } = &mut s.page {
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                *edit_cursor = edit_cursor.prev();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                *edit_cursor = edit_cursor.next();
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Enter => {
                // Handle different actions based on selected cursor
                handle_enter_key(&snapshot, selected_cursor, bus)
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                // Handle left/right for option switching on selector fields
                handle_option_switch(&snapshot, selected_cursor, key.code, bus)
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

/// Handle Enter key press based on the selected cursor
fn handle_enter_key(snapshot: &types::Status, selected_cursor: types::ui::ConfigPanelCursor, bus: &Bus) -> Result<()> {
    match selected_cursor {
        types::ui::ConfigPanelCursor::EnablePort => {
            // Enable Port toggle - attempt to toggle port state
            handle_port_toggle(snapshot, bus)
        }
        types::ui::ConfigPanelCursor::ProtocolMode => {
            // Protocol mode selection - could implement mode switching here
            // For now, just do nothing or show a message
            Ok(())
        }
        types::ui::ConfigPanelCursor::ProtocolConfig => {
            // Protocol configuration - navigate to the appropriate config page
            handle_protocol_navigation(snapshot, bus)
        }
        types::ui::ConfigPanelCursor::BaudRate |
        types::ui::ConfigPanelCursor::DataBits |
        types::ui::ConfigPanelCursor::Parity |
        types::ui::ConfigPanelCursor::StopBits => {
            // Serial parameter fields - enter edit mode
            handle_parameter_edit(snapshot, selected_cursor, bus)
        }
    }
}

/// Handle left/right key presses for option switching
fn handle_option_switch(
    snapshot: &types::Status,
    selected_cursor: types::ui::ConfigPanelCursor,
    key_code: KeyCode,
    bus: &Bus,
) -> Result<()> {
    match selected_cursor {
        types::ui::ConfigPanelCursor::ProtocolMode => {
            // Switch between Modbus and MQTT
            // TODO: Implement protocol mode switching
            // For now, just refresh
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        types::ui::ConfigPanelCursor::Parity => {
            // Switch between None/Odd/Even parity
            handle_parity_switch(snapshot, key_code, bus)
        }
        _ => Ok(()), // Other fields don't support left/right switching
    }
}

/// Handle parity switching with left/right keys
fn handle_parity_switch(snapshot: &types::Status, key_code: KeyCode, bus: &Bus) -> Result<()> {
    if let types::Page::ModbusConfig { selected_port, .. } = &snapshot.page {
        if let Some(port_name) = snapshot.ports.order.get(*selected_port) {
            write_status(|s| {
                if let Some(pd) = s.ports.map.get_mut(port_name) {
                    if let Some(rt) = pd.runtime.as_mut() {
                        let current_parity = rt.current_cfg.parity;
                        rt.current_cfg.parity = match key_code {
                            KeyCode::Left | KeyCode::Char('h') => {
                                match current_parity {
                                    serialport::Parity::None => serialport::Parity::Even,
                                    serialport::Parity::Odd => serialport::Parity::None,
                                    serialport::Parity::Even => serialport::Parity::Odd,
                                }
                            }
                            KeyCode::Right | KeyCode::Char('l') => {
                                match current_parity {
                                    serialport::Parity::None => serialport::Parity::Odd,
                                    serialport::Parity::Odd => serialport::Parity::Even,
                                    serialport::Parity::Even => serialport::Parity::None,
                                }
                            }
                            _ => current_parity,
                        };
                    }
                }
                Ok(())
            })?;
        }
    }

    bus.ui_tx
        .send(crate::tui::utils::bus::UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
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
                                port_data.state =
                                    crate::protocol::status::types::port::PortState::OccupiedByThis;
                            }
                            Ok(())
                        })?;
                    }
                    crate::protocol::status::types::port::PortState::OccupiedByThis => {
                        // Port is occupied by us, disable it
                        write_status(|s| {
                            if let Some(port_data) = s.ports.map.get_mut(port_name) {
                                port_data.state =
                                    crate::protocol::status::types::port::PortState::Free;
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

/// Handle protocol configuration navigation
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
fn handle_parameter_edit(snapshot: &types::Status, selected_cursor: types::ui::ConfigPanelCursor, bus: &Bus) -> Result<()> {
    if let types::Page::ModbusConfig { selected_port, .. } = &snapshot.page {
        if let Some(port_name) = snapshot.ports.order.get(*selected_port) {
            if let Some(pd) = snapshot.ports.map.get(port_name) {
                // Get initial value for the field being edited
                let init_buf = get_field_initial_value_by_cursor(pd, selected_cursor);

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
                        *edit_cursor = selected_cursor;
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

/// Get initial value for editing a specific field by cursor type
fn get_field_initial_value_by_cursor(
    pd: &crate::protocol::status::types::port::PortData,
    cursor_type: types::ui::ConfigPanelCursor,
) -> String {
    match cursor_type {
        types::ui::ConfigPanelCursor::BaudRate => {
            pd.runtime
                .as_ref()
                .map(|rt| rt.current_cfg.baud.to_string())
                .unwrap_or_else(|| "9600".to_string())
        }
        types::ui::ConfigPanelCursor::DataBits => {
            pd.runtime
                .as_ref()
                .map(|rt| rt.current_cfg.data_bits.to_string())
                .unwrap_or_else(|| "8".to_string())
        }
        types::ui::ConfigPanelCursor::Parity => {
            pd.runtime
                .as_ref()
                .map(|rt| format!("{:?}", rt.current_cfg.parity))
                .unwrap_or_else(|| lang().protocol.common.parity_none.clone())
        }
        types::ui::ConfigPanelCursor::StopBits => {
            pd.runtime
                .as_ref()
                .map(|rt| rt.current_cfg.stop_bits.to_string())
                .unwrap_or_else(|| "1".to_string())
        }
        _ => String::new(),
    }
}