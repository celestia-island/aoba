use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::types::cursor::Cursor,
    protocol::status::{read_status, types, write_status},
    tui::utils::bus::Bus,
};

/// Handle input for config panel. Sends commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Create a snapshot of the current status (previously passed as `app`)
    let snapshot = read_status(|s| Ok(s.clone()))?;
    // Derive selected cursor in panel
    let selected_cursor = super::components::derive_selection(&snapshot);

    // Check if we're in edit mode (simplified - using global buffer)
    let in_edit = !snapshot.temporarily.input_raw_buffer.is_empty();

    if in_edit {
        // We are editing a field: handle editing keys (but not character input - that's handled globally)
        match key.code {
            KeyCode::Enter => {
                // Commit edit: write buffer back to appropriate field
                let buffer_content = snapshot.temporarily.input_raw_buffer.clone();

                write_status(|s| {
                    // Clear the global buffer
                    s.temporarily.input_raw_buffer.clear();

                    // Apply the edit based on current cursor
                    if let types::Page::ConfigPanel {
                        cursor,
                        selected_port,
                        ..
                    } = &s.page
                    {
                        if let Some(port_name) = s.ports.order.get(*selected_port) {
                            if let Some(pd) = s.ports.map.get_mut(port_name) {
                                match cursor {
                                    types::cursor::ConfigPanelCursor::BaudRate => {
                                        if let Ok(baud) = buffer_content.parse::<u32>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.baud = baud;
                                            }
                                        }
                                    }
                                    types::cursor::ConfigPanelCursor::DataBits => {
                                        if let Ok(bits) = buffer_content.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                match bits {
                                                    5 => rt.current_cfg.data_bits = 5,
                                                    6 => rt.current_cfg.data_bits = 6,
                                                    7 => rt.current_cfg.data_bits = 7,
                                                    8 => rt.current_cfg.data_bits = 8,
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                    types::cursor::ConfigPanelCursor::StopBits => {
                                        if let Ok(bits) = buffer_content.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                match bits {
                                                    1 => rt.current_cfg.stop_bits = 1u8,
                                                    2 => rt.current_cfg.stop_bits = 2u8,
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                    // Other fields can be handled here
                                    _ => {}
                                }
                            }
                        }
                    }
                    Ok(())
                })?;

                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Esc => {
                // Cancel edit: clear buffer
                write_status(|s| {
                    s.temporarily.input_raw_buffer.clear();
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Backspace => {
                write_status(|s| {
                    s.temporarily.input_raw_buffer.pop();
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Delete => {
                // For simplicity, treat delete as backspace in this context
                write_status(|s| {
                    s.temporarily.input_raw_buffer.pop();
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
        // Not in edit mode: handle navigation and actions
        match key.code {
            KeyCode::PageUp => {
                // Scroll up
                super::components::config_panel_scroll_up(5)?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::PageDown => {
                // Scroll down
                super::components::config_panel_scroll_down(5)?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
                // Navigate between fields using cursor system
                write_status(|s| {
                    if let types::Page::ConfigPanel {
                        cursor,
                        view_offset,
                        ..
                    } = &mut s.page
                    {
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                *cursor = cursor.prev();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                *cursor = cursor.next();
                            }
                            _ => {}
                        }
                        // Recompute view offset using the cursor's index mapping
                        *view_offset = cursor.view_offset();
                    }
                    Ok(())
                })?;

                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                // Handle option switching for certain fields
                match selected_cursor {
                    types::cursor::ConfigPanelCursor::Parity => {
                        // Cycle through parity options
                        write_status(|s| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &s.page {
                                if let Some(port_name) = s.ports.order.get(*selected_port) {
                                    if let Some(pd) = s.ports.map.get_mut(port_name) {
                                        if let Some(rt) = pd.runtime.as_mut() {
                                            rt.current_cfg.parity = match rt.current_cfg.parity {
                                                serialport::Parity::None => serialport::Parity::Odd,
                                                serialport::Parity::Odd => serialport::Parity::Even,
                                                serialport::Parity::Even => {
                                                    serialport::Parity::None
                                                }
                                            };
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })?;
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|err| anyhow!(err))?;
                    }
                    _ => {}
                }
                Ok(())
            }
            KeyCode::Enter => {
                // Handle Enter key for different cursor positions
                match selected_cursor {
                    types::cursor::ConfigPanelCursor::EnablePort => {
                        // Toggle port enable/disable
                        // TODO: Implement port toggle with error handling
                        Ok(())
                    }
                    types::cursor::ConfigPanelCursor::ProtocolConfig => {
                        // Navigate to modbus panel
                        write_status(|s| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &s.page {
                                s.page = types::Page::ModbusDashboard {
                                    selected_port: *selected_port,
                                    view_offset: 0,
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
                            }
                            Ok(())
                        })?;
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|err| anyhow!(err))?;
                        Ok(())
                    }
                    types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
                        // Navigate to log panel
                        write_status(|s| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &s.page {
                                s.page = types::Page::LogPanel {
                                    selected_port: *selected_port,
                                    input_mode: types::ui::InputMode::Ascii,
                                    view_offset: 0,
                                };
                            }
                            Ok(())
                        })?;
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|err| anyhow!(err))?;
                        Ok(())
                    }
                    types::cursor::ConfigPanelCursor::BaudRate
                    | types::cursor::ConfigPanelCursor::DataBits
                    | types::cursor::ConfigPanelCursor::StopBits => {
                        // Enter edit mode: initialize buffer with current value
                        write_status(|s| {
                            if let types::Page::ConfigPanel {
                                selected_port,
                                cursor,
                                ..
                            } = &s.page
                            {
                                if let Some(port_name) = s.ports.order.get(*selected_port) {
                                    if let Some(pd) = s.ports.map.get(port_name) {
                                        let init_value = match cursor {
                                            types::cursor::ConfigPanelCursor::BaudRate => pd
                                                .runtime
                                                .as_ref()
                                                .map(|rt| rt.current_cfg.baud.to_string())
                                                .unwrap_or_else(|| "9600".to_string()),
                                            types::cursor::ConfigPanelCursor::DataBits => pd
                                                .runtime
                                                .as_ref()
                                                .map(|rt| rt.current_cfg.data_bits.to_string())
                                                .unwrap_or_else(|| "8".to_string()),
                                            types::cursor::ConfigPanelCursor::StopBits => pd
                                                .runtime
                                                .as_ref()
                                                .map(|rt| rt.current_cfg.stop_bits.to_string())
                                                .unwrap_or_else(|| "1".to_string()),
                                            _ => String::new(),
                                        };
                                        s.temporarily.input_raw_buffer = init_value;
                                    }
                                }
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
            }
            KeyCode::Esc => {
                // Return to entry page
                let cursor = if let types::Page::ConfigPanel { selected_port, .. } = &snapshot.page
                {
                    Some(types::cursor::EntryCursor::Com {
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
