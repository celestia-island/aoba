use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::utils::bus::Bus,
};

/// Handle input for config panel. Sends commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Create a snapshot of the current status (previously passed as `app`)
    let snapshot = read_status(|s| Ok(s.clone()))?;
    // Derive selected row in panel (same logic as render_kv_panel)
    let selected_row = super::components::derive_selection(&snapshot);

    // Determine number of fields shown in panel
    let labels_count = {
        let base = 5usize;
        let more = 9usize;
        base + more
    };

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
                let _ = crate::protocol::status::write_status(|s| {
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
                });
                Ok(())
            }
            KeyCode::Backspace => {
                let _ = crate::protocol::status::write_status(|s| {
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
                });
                Ok(())
            }
            KeyCode::Left => {
                let _ = crate::protocol::status::write_status(|s| {
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
                });
                Ok(())
            }
            KeyCode::Right => {
                let _ = crate::protocol::status::write_status(|s| {
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
                });
                Ok(())
            }
            KeyCode::Enter => {
                // Commit edit: write buffer back to PortData field
                let _ = crate::protocol::status::write_status(|s| {
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
                                    0 => pd.port_name = val,
                                    1 => {
                                        if let Ok(v) = val.parse::<u32>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.baud = v;
                                            }
                                        }
                                    }
                                    2 => {
                                        if let Ok(v) = val.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.data_bits = v;
                                            }
                                        }
                                    }
                                    3 => {
                                        if let Some(rt) = pd.runtime.as_mut() {
                                            rt.current_cfg.parity = match val.as_str() {
                                                "None" | "none" => serialport::Parity::None,
                                                "Odd" | "odd" => serialport::Parity::Odd,
                                                "Even" | "even" => serialport::Parity::Even,
                                                _ => rt.current_cfg.parity,
                                            }
                                        }
                                    }
                                    4 => {
                                        if let Ok(v) = val.parse::<u8>() {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.stop_bits = v;
                                            }
                                        }
                                    }
                                    5 => pd.port_type = val,
                                    6 => {
                                        pd.extra.guid =
                                            if val.is_empty() { None } else { Some(val) }
                                    }
                                    7 => {}
                                    8 => {
                                        pd.extra.serial =
                                            if val.is_empty() { None } else { Some(val) }
                                    }
                                    9 => {
                                        pd.extra.manufacturer =
                                            if val.is_empty() { None } else { Some(val) }
                                    }
                                    10 => {
                                        pd.extra.product =
                                            if val.is_empty() { None } else { Some(val) }
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
                });
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
                Ok(())
            }
            KeyCode::Esc => {
                // Cancel edit
                let _ = crate::protocol::status::write_status(|s| {
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
                });
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
                Ok(())
            }
            _ => Ok(()),
        }
    } else {
        // Not in edit mode: handle navigation and enter/e to begin editing
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
                // Update selected_port inside Page::ModbusConfig under write lock
                let _ = crate::protocol::status::write_status(|s| {
                    if let types::Page::ModbusConfig { selected_port, .. } = &mut s.page {
                        // Move selection by delta based on key
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                if *selected_port > 0 {
                                    *selected_port = selected_port.saturating_sub(1);
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let max = s.ports.order.len().saturating_sub(1);
                                if *selected_port < max {
                                    *selected_port += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                });
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
                Ok(())
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                // Begin edit for selected row if a real port exists at selection
                // Determine selected port name from app snapshot
                let sel_idx = selected_row.min(labels_count.saturating_sub(1));
                if let Some(port_name) = snapshot.ports.order.get(sel_idx) {
                    // Initialize config_edit in Status
                    let init_buf = if let Some(pd) = snapshot.ports.map.get(port_name) {
                        // Determine value to prefill based on sel_idx (same mapping as render)
                        let pre = match sel_idx {
                            0 => pd.port_name.clone(),
                            1 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| rt.current_cfg.baud.to_string())
                                .unwrap_or_else(|| "9600".to_string()),
                            2 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| rt.current_cfg.data_bits.to_string())
                                .unwrap_or_else(|| "8".to_string()),
                            3 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| format!("{:?}", rt.current_cfg.parity))
                                .unwrap_or_else(|| lang().protocol.common.parity_none.clone()),
                            4 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| rt.current_cfg.stop_bits.to_string())
                                .unwrap_or_else(|| "1".to_string()),
                            5 => pd.port_type.clone(),
                            6 => pd.extra.guid.clone().unwrap_or_default(),
                            7 => match (pd.extra.vid, pd.extra.pid) {
                                (Some(vid), Some(pid)) => format!("{vid:04x}:{pid:04x}"),
                                _ => String::new(),
                            },
                            8 => pd.extra.serial.clone().unwrap_or_default(),
                            9 => pd.extra.manufacturer.clone().unwrap_or_default(),
                            10 => pd.extra.product.clone().unwrap_or_default(),
                            _ => String::new(),
                        };
                        pre
                    } else {
                        String::new()
                    };

                    let _ = crate::protocol::status::write_status(|s| {
                        if let types::Page::ModbusConfig {
                            edit_active: config_edit_active,
                            edit_cursor: config_edit_cursor,
                            edit_buffer: config_edit_buffer,
                            edit_cursor_pos: config_edit_cursor_pos,
                            ..
                        } = &mut s.page
                        {
                            *config_edit_active = true;
                            *config_edit_cursor = selected_row;
                            *config_edit_buffer = init_buf.clone();
                            *config_edit_cursor_pos = init_buf.len();
                        }
                        Ok(())
                    });
                    bus.ui_tx
                        .send(crate::tui::utils::bus::UiToCore::Refresh)
                        .map_err(|e| anyhow!(e))?;
                    Ok(())
                } else {
                    // No port under selection: just refresh
                    bus.ui_tx
                        .send(crate::tui::utils::bus::UiToCore::Refresh)
                        .map_err(|e| anyhow!(e))?;
                    Ok(())
                }
            }
            KeyCode::Esc => {
                // If we reach here we are not in per-field edit mode (in_edit == false)
                // so Esc should return the user to the main entry page.
                let _ = crate::protocol::status::write_status(|s| {
                    s.page = types::Page::Entry { cursor: None };
                    Ok(())
                });
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
