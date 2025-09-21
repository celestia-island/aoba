use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{self, cursor::Cursor},
        with_port_read, with_port_write, write_status,
    },
    tui::utils::bus::Bus,
};

/// Ensure current cursor for ConfigPanel does not point to hidden items when
/// the selected port is not occupied by this instance. This moves the cursor
/// to a visible default (`EnablePort`) and updates `view_offset` when needed.
fn sanitize_configpanel_cursor() -> Result<()> {
    write_status(|status| {
        if let types::Page::ConfigPanel {
            cursor,
            selected_port,
            view_offset,
            ..
        } = &mut status.page
        {
            // Determine occupancy of selected port
            let occupied = if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Some(b) = with_port_read(port, |port| {
                        matches!(port.state, types::port::PortState::OccupiedByThis { .. })
                    }) {
                        b
                    } else {
                        log::warn!(
                            "sanitize_configpanel_cursor: failed to acquire read lock for {}",
                            port_name
                        );
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !occupied {
                match cursor {
                    types::cursor::ConfigPanelCursor::EnablePort
                    | types::cursor::ConfigPanelCursor::ProtocolMode => {
                        // allowed
                    }
                    _ => {
                        *cursor = types::cursor::ConfigPanelCursor::EnablePort;
                    }
                }

                *view_offset = cursor.view_offset();
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the ConfigPanel view offset up by `amount` (saturating at 0).
pub fn handle_scroll_up(amount: usize) -> Result<()> {
    write_status(|status| {
        if let types::Page::ConfigPanel { view_offset, .. } = &mut status.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the ConfigPanel view offset down by `amount`.
pub fn handle_scroll_down(amount: usize) -> Result<()> {
    write_status(|status| {
        if let types::Page::ConfigPanel { view_offset, .. } = &mut status.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Derive selected cursor in panel
    let selected_cursor = super::components::derive_selection()?;

    // Ensure cursor does not point to hidden items on entry
    sanitize_configpanel_cursor()?;

    // Check if we're in edit mode (simplified - using global buffer)
    let in_edit = read_status(|status| Ok(!status.temporarily.input_raw_buffer.is_empty()))?;

    if in_edit {
        // We are editing a field: handle editing keys (but not character input - that's handled globally)
        match key.code {
            KeyCode::Enter => {
                // Commit edit: write buffer back to appropriate field
                let buffer_content =
                    read_status(|status| match &status.temporarily.input_raw_buffer {
                        types::ui::InputRawBuffer::String(value) => {
                            Ok(String::from_utf8_lossy(value).into_owned())
                        }
                        types::ui::InputRawBuffer::Index(index) => Ok(index.to_string()),
                        types::ui::InputRawBuffer::None => Ok(String::new()),
                    })?;

                write_status(|status| {
                    // Clear the global buffer
                    status.temporarily.input_raw_buffer.clear();

                    // Apply the edit based on current cursor
                    if let types::Page::ConfigPanel {
                        cursor,
                        selected_port,
                        ..
                    } = &status.page
                    {
                        if let Some(port_name) = status.ports.order.get(*selected_port) {
                            if let Some(port) = status.ports.map.get(port_name) {
                                if let Some(_) = with_port_write(port, |port| {
                                    match cursor {
                                        types::cursor::ConfigPanelCursor::BaudRate => {
                                            if let Ok(baud) = buffer_content.parse::<u32>() {
                                                if let types::port::PortState::OccupiedByThis {
                                                    runtime,
                                                    ..
                                                } = &mut port.state
                                                {
                                                    runtime.current_cfg.baud = baud;
                                                }
                                            }
                                        }
                                        types::cursor::ConfigPanelCursor::DataBits => {
                                            if let Ok(bits) = buffer_content.parse::<u8>() {
                                                if let types::port::PortState::OccupiedByThis {
                                                    runtime,
                                                    ..
                                                } = &mut port.state
                                                {
                                                    match bits {
                                                        5 => runtime.current_cfg.data_bits = 5,
                                                        6 => runtime.current_cfg.data_bits = 6,
                                                        7 => runtime.current_cfg.data_bits = 7,
                                                        8 => runtime.current_cfg.data_bits = 8,
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        types::cursor::ConfigPanelCursor::StopBits => {
                                            if let Ok(bits) = buffer_content.parse::<u8>() {
                                                if let types::port::PortState::OccupiedByThis {
                                                    runtime,
                                                    ..
                                                } = &mut port.state
                                                {
                                                    match bits {
                                                        1 => runtime.current_cfg.stop_bits = 1u8,
                                                        2 => runtime.current_cfg.stop_bits = 2u8,
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        // Other fields can be handled here
                                        _ => {}
                                    }
                                }) {
                                    // updated
                                } else {
                                    log::warn!(
                                        "handle_input: failed to acquire write lock for {}",
                                        port_name
                                    );
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
                write_status(|status| {
                    status.temporarily.input_raw_buffer.clear();
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Backspace => {
                write_status(|status| {
                    status.temporarily.input_raw_buffer.pop();
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Delete => {
                // For simplicity, treat delete as backspace in this context
                write_status(|status| {
                    status.temporarily.input_raw_buffer.pop();
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
                handle_scroll_up(5)?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::PageDown => {
                // Scroll down
                handle_scroll_down(5)?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
                // Navigate between fields using cursor system

                write_status(|status| {
                    if let types::Page::ConfigPanel {
                        cursor,
                        view_offset,
                        ..
                    } = &mut status.page
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

                // After moving cursor, sanitize again to ensure we didn't land on a hidden item
                sanitize_configpanel_cursor()?;

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
                        write_status(|status| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &status.page {
                                if let Some(port_name) = status.ports.order.get(*selected_port) {
                                    if let Some(port) = status.ports.map.get(port_name) {
                                        if let Some(_) = with_port_write(port, |port| {
                                            if let types::port::PortState::OccupiedByThis {
                                                runtime,
                                                ..
                                            } = &mut port.state
                                            {
                                                runtime.current_cfg.parity =
                                                    match runtime.current_cfg.parity {
                                                        serialport::Parity::None => {
                                                            serialport::Parity::Odd
                                                        }
                                                        serialport::Parity::Odd => {
                                                            serialport::Parity::Even
                                                        }
                                                        serialport::Parity::Even => {
                                                            serialport::Parity::None
                                                        }
                                                    };
                                            }
                                        }) {
                                            // updated
                                        } else {
                                            log::warn!("handle_input: failed to acquire write lock for {} (parity)", port_name);
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
                        // Toggle port enable/disable: send a ToggleRuntime message to core
                        // Determine selected port name and request toggle
                        if let Some(port_name) = read_status(|status| {
                            if let types::Page::ConfigPanel { selected_port, .. } = status.page {
                                Ok(status.ports.order.get(selected_port).cloned())
                            } else {
                                Ok(None)
                            }
                        })? {
                            bus.ui_tx
                                .send(crate::tui::utils::bus::UiToCore::ToggleRuntime(port_name))
                                .map_err(|err| anyhow!(err))?;
                        }
                        Ok(())
                    }
                    types::cursor::ConfigPanelCursor::ProtocolConfig => {
                        // Navigate to modbus panel
                        write_status(|status| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &status.page {
                                status.page = types::Page::ModbusDashboard {
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
                        write_status(|status| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &status.page {
                                status.page = types::Page::LogPanel {
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
                        write_status(|status| {
                            if let types::Page::ConfigPanel {
                                selected_port,
                                cursor,
                                ..
                            } = &status.page
                            {
                                if let Some(port_name) = status.ports.order.get(*selected_port) {
                                    if let Some(pd_arc) = status.ports.map.get(port_name) {
                                        let init_value = with_port_read(
                                            pd_arc,
                                            |pd| match cursor {
                                                types::cursor::ConfigPanelCursor::BaudRate => match &pd.state {
                                                    types::port::PortState::OccupiedByThis { runtime, .. } => {
                                                        runtime.current_cfg.baud.to_string()
                                                    }
                                                    _ => "9600".to_string(),
                                                },
                                                types::cursor::ConfigPanelCursor::DataBits => match &pd.state {
                                                    types::port::PortState::OccupiedByThis { runtime, .. } => {
                                                        runtime.current_cfg.data_bits.to_string()
                                                    }
                                                    _ => "8".to_string(),
                                                },
                                                types::cursor::ConfigPanelCursor::StopBits => match &pd.state {
                                                    types::port::PortState::OccupiedByThis { runtime, .. } => {
                                                        runtime.current_cfg.stop_bits.to_string()
                                                    }
                                                    _ => "1".to_string(),
                                                },
                                                _ => String::new(),
                                            },
                                        )
                                        .unwrap_or_else(|| String::new());

                                        status.temporarily.input_raw_buffer =
                                            types::ui::InputRawBuffer::String(
                                                init_value.into_bytes(),
                                            );
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
                let cursor = read_status(|status| {
                    if let types::Page::ConfigPanel { selected_port, .. } = status.page {
                        Ok(Some(types::cursor::EntryCursor::Com { idx: selected_port }))
                    } else {
                        Ok(None)
                    }
                })?;

                write_status(|status| {
                    status.page = types::Page::Entry { cursor };
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
