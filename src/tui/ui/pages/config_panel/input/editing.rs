use anyhow::{anyhow, Result};
use strum::IntoEnumIterator;

use crossterm::event::{KeyCode, KeyEvent};

use super::navigation::sanitize_configpanel_cursor;
use crate::{
    core::bus::{self, Bus},
    tui::{
        status as types,
        status::{cursor::Cursor, read_status, write_status},
    },
    utils::i18n::lang,
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    log::info!("ConfigPanel::handle_input: key={:?}", key.code);
    let selected_cursor = super::super::components::derive_selection()?;

    sanitize_configpanel_cursor()?;

    // Check if we're in editing mode: buffer should not be None
    // (even if bytes are empty, we're still editing)
    let in_edit = read_status(|status| {
        Ok(!matches!(
            status.temporarily.input_raw_buffer,
            types::ui::InputRawBuffer::None
        ))
    })?;
    log::info!("ConfigPanel::handle_input: in_edit={in_edit}, cursor={selected_cursor:?}");

    if in_edit {
        // Handle editing mode with proper input span handler
        handle_editing_input(key, bus, selected_cursor)?;
    } else {
        // Handle navigation mode
        handle_navigation_input(key, bus, selected_cursor)?;
    }
    Ok(())
}

fn handle_editing_input(
    key: KeyEvent,
    bus: &Bus,
    selected_cursor: types::cursor::ConfigPanelCursor,
) -> Result<()> {
    let index_choices: Option<usize> = match selected_cursor {
        types::cursor::ConfigPanelCursor::BaudRate => {
            Some(types::modbus::BaudRateSelector::iter().count())
        }
        types::cursor::ConfigPanelCursor::DataBits { .. } => {
            Some(types::modbus::DataBitsOption::iter().count())
        }
        types::cursor::ConfigPanelCursor::StopBits => {
            Some(types::modbus::StopBitsOption::iter().count())
        }
        types::cursor::ConfigPanelCursor::Parity => {
            Some(types::modbus::ParityOption::iter().count())
        }
        types::cursor::ConfigPanelCursor::ProtocolMode => {
            Some(1) // Only one option: Modbus RTU for now
        }
        _ => None,
    };

    // For BaudRate (custom numeric input) only allow numeric input during editing
    if matches!(selected_cursor, types::cursor::ConfigPanelCursor::BaudRate) {
        // Numeric input only for timeout/interval fields
        crate::tui::ui::components::input_span_handler::handle_input_span(
            key,
            bus,
            index_choices,
            None,
            |c: char| c.is_ascii_digit(),
            |maybe_string| -> Result<()> {
                let port_name_opt = read_status(|status| {
                    if let crate::tui::status::Page::ConfigPanel { selected_port, .. } = status.page
                    {
                        Ok(status.ports.order.get(selected_port).cloned())
                    } else {
                        Ok(None)
                    }
                })?;

                if let Some(port_name) = port_name_opt {
                    if let Some(_port) =
                        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                    {
                        if let Some(s) = maybe_string {
                            if selected_cursor == types::cursor::ConfigPanelCursor::BaudRate {
                                if let Ok(parsed) = s.trim().parse::<u32>() {
                                    if (1000..=2_000_000).contains(&parsed) {
                                        // Update serial config directly in status
                                        write_status(|status| {
                                            if let Some(port) = status.ports.map.get_mut(&port_name)
                                            {
                                                port.serial_config.baud = parsed;
                                                port.config_modified = true;
                                            }
                                            Ok(())
                                        })?;
                                    } else {
                                        log::warn!("Custom baud is out of allowed range: {parsed}");
                                    }
                                } else {
                                    log::warn!("Failed to parse custom baud value: {s}");
                                }
                            }

                            write_status(|status| {
                                status.temporarily.input_raw_buffer.clear();
                                Ok(())
                            })?;
                        } else {
                            // Handle selector edits
                            handle_selector_commit(&port_name, selected_cursor)?;
                        }
                    }
                }

                bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
                Ok(())
            },
        )?;
    } else {
        // Accept all characters for other fields (non-numeric fields)
        crate::tui::ui::components::input_span_handler::handle_input_span(
            key,
            bus,
            index_choices,
            None,
            |_: char| true,
            |maybe_string| -> Result<()> {
                let port_name_opt = read_status(|status| {
                    if let crate::tui::status::Page::ConfigPanel { selected_port, .. } = status.page
                    {
                        Ok(status.ports.order.get(selected_port).cloned())
                    } else {
                        Ok(None)
                    }
                })?;

                if let Some(port_name) = port_name_opt {
                    if let Some(_port) =
                        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                    {
                        if let Some(s) = maybe_string {
                            if selected_cursor == types::cursor::ConfigPanelCursor::BaudRate {
                                if let Ok(parsed) = s.trim().parse::<u32>() {
                                    if (1000..=2_000_000).contains(&parsed) {
                                        // Update serial config directly in status
                                        write_status(|status| {
                                            if let Some(port) = status.ports.map.get_mut(&port_name)
                                            {
                                                port.serial_config.baud = parsed;
                                                port.config_modified = true;
                                            }
                                            Ok(())
                                        })?;
                                    } else {
                                        log::warn!("Custom baud is out of allowed range: {parsed}");
                                    }
                                } else {
                                    log::warn!("Failed to parse custom baud value: {s}");
                                }
                            }

                            write_status(|status| {
                                status.temporarily.input_raw_buffer.clear();
                                Ok(())
                            })?;
                        } else {
                            // Handle selector edits
                            handle_selector_commit(&port_name, selected_cursor)?;
                        }
                    }
                }

                bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
                Ok(())
            },
        )?;
    }
    Ok(())
}

fn handle_navigation_input(
    key: KeyEvent,
    bus: &Bus,
    selected_cursor: types::cursor::ConfigPanelCursor,
) -> Result<()> {
    log::info!(
        "handle_navigation_input: key={:?}, cursor={:?}",
        key.code,
        selected_cursor
    );
    match key.code {
        KeyCode::PageUp => {
            // Jump to first cursor position (EnablePort)
            let new_cursor = types::cursor::ConfigPanelCursor::EnablePort;
            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let crate::tui::status::Page::ConfigPanel {
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
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::PageDown => {
            // Jump to last cursor position (StopBits)
            let new_cursor = types::cursor::ConfigPanelCursor::StopBits;
            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let crate::tui::status::Page::ConfigPanel {
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
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
            write_status(|status| {
                if let crate::tui::status::Page::ConfigPanel {
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
                    *view_offset = cursor.view_offset();
                }
                Ok(())
            })?;

            sanitize_configpanel_cursor()?;

            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => Ok(()),
        KeyCode::Enter => {
            log::info!("handle_navigation_input: Enter pressed, calling handle_enter_action");
            handle_enter_action(selected_cursor, bus)?;
            log::info!("handle_navigation_input: handle_enter_action completed");
            Ok(())
        }
        KeyCode::Esc => {
            // Return to entry page
            // First, get the selected_port and ports_count outside the write lock
            let (selected_port_opt, _ports_count) = read_status(|status| {
                let selected_port =
                    if let crate::tui::status::Page::ConfigPanel { selected_port, .. } =
                        &status.page
                    {
                        Some(*selected_port)
                    } else {
                        None
                    };
                Ok((selected_port, status.ports.order.len()))
            })?;

            write_status(|status| {
                if let Some(selected_port) = selected_port_opt {
                    let new_cursor = types::cursor::EntryCursor::Com {
                        index: selected_port,
                    };
                    status.page = crate::tui::status::Page::Entry {
                        cursor: Some(new_cursor),
                        view_offset: selected_port, // For Com cursor, offset equals index
                    };
                } else {
                    status.page = crate::tui::status::Page::Entry {
                        cursor: None,
                        view_offset: 0,
                    };
                }
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_enter_action(selected_cursor: types::cursor::ConfigPanelCursor, bus: &Bus) -> Result<()> {
    log::info!("handle_enter_action called, cursor={selected_cursor:?}");
    match selected_cursor {
        types::cursor::ConfigPanelCursor::EnablePort => {
            log::info!("EnablePort case matched");
            log::info!("🔘 User pressed Enter on EnablePort in ConfigPanel");
            if let Some(port_name) = read_status(|status| {
                if let crate::tui::status::Page::ConfigPanel { selected_port, .. } = status.page {
                    Ok(status.ports.order.get(selected_port).cloned())
                } else {
                    Ok(None)
                }
            })? {
                // Check if this is a Modbus port with empty configuration
                let is_modbus_empty = read_status(|status| {
                    if let Some(port) = status.ports.map.get(&port_name) {
                        let port_data = port;
                        let types::port::PortConfig::Modbus { stations, .. } = &port_data.config;

                        return Ok(stations.is_empty());
                    }
                    Ok(false)
                })?;

                if is_modbus_empty {
                    log::warn!("⚠️  Cannot enable Modbus port with empty configuration");
                    write_status(|status| {
                        status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                            message: lang().index.err_modbus_config_empty.clone(),
                            timestamp: chrono::Local::now(),
                        });
                        Ok(())
                    })?;
                    bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
                    return Ok(());
                }

                log::info!("Sending ToggleRuntime for port: {port_name}");
                log::info!("📤 Sending ToggleRuntime({port_name}) message to core");
                bus.ui_tx
                    .send(crate::core::bus::UiToCore::ToggleRuntime(port_name.clone()))
                    .map_err(|err| anyhow!(err))?;
                log::info!("✅ ToggleRuntime({port_name}) message sent successfully");
            } else {
                log::warn!("port_name is None");
                log::warn!("⚠️  EnablePort action failed: port_name is None");
            }
            Ok(())
        }
        types::cursor::ConfigPanelCursor::ProtocolConfig => {
            write_status(|status| {
                if let crate::tui::status::Page::ConfigPanel { selected_port, .. } = &status.page {
                    status.page = crate::tui::status::Page::ModbusDashboard {
                        selected_port: *selected_port,
                        view_offset: 0,
                        cursor: types::cursor::ModbusDashboardCursor::AddLine,
                    };
                }
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
            Ok(())
        }
        types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
            write_status(|status| {
                if let crate::tui::status::Page::ConfigPanel { selected_port, .. } = &status.page {
                    status.page = crate::tui::status::Page::LogPanel {
                        selected_port: *selected_port,
                        input_mode: types::ui::InputMode::Ascii,
                        selected_item: None,
                    };
                }
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
            Ok(())
        }
        types::cursor::ConfigPanelCursor::BaudRate
        | types::cursor::ConfigPanelCursor::DataBits { .. }
        | types::cursor::ConfigPanelCursor::StopBits
        | types::cursor::ConfigPanelCursor::Parity => {
            start_editing_mode(selected_cursor)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn start_editing_mode(_selected_cursor: types::cursor::ConfigPanelCursor) -> Result<()> {
    write_status(|status| {
        status.temporarily.input_raw_buffer.clear();
        Ok(())
    })?;

    write_status(|status| {
        if let crate::tui::status::Page::ConfigPanel {
            selected_port,
            cursor,
            ..
        } = &status.page
        {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    match cursor {
                        types::cursor::ConfigPanelCursor::BaudRate => {
                            let index =
                                types::modbus::BaudRateSelector::from_u32(port.serial_config.baud)
                                    .to_index();

                            status.temporarily.input_raw_buffer =
                                types::ui::InputRawBuffer::Index(index);
                        }
                        types::cursor::ConfigPanelCursor::DataBits { .. } => {
                            let index = match port.serial_config.data_bits {
                                5 => 0usize,
                                6 => 1usize,
                                7 => 2usize,
                                _ => 3usize,
                            };

                            status.temporarily.input_raw_buffer =
                                types::ui::InputRawBuffer::Index(index);
                        }
                        types::cursor::ConfigPanelCursor::StopBits => {
                            let index = match port.serial_config.stop_bits {
                                1 => 0usize,
                                _ => 1usize,
                            };

                            status.temporarily.input_raw_buffer =
                                types::ui::InputRawBuffer::Index(index);
                        }
                        types::cursor::ConfigPanelCursor::Parity => {
                            let index = match port.serial_config.parity {
                                types::port::SerialParity::None => 0usize,
                                types::port::SerialParity::Odd => 1usize,
                                types::port::SerialParity::Even => 2usize,
                            };

                            status.temporarily.input_raw_buffer =
                                types::ui::InputRawBuffer::Index(index);
                        }
                        types::cursor::ConfigPanelCursor::ProtocolMode => {
                            // Only one option for now: Modbus RTU
                            status.temporarily.input_raw_buffer =
                                types::ui::InputRawBuffer::Index(0);
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}

fn handle_selector_commit(
    port_name: &str,
    selected_cursor: types::cursor::ConfigPanelCursor,
) -> Result<()> {
    let idx = read_status(|status| Ok(status.temporarily.input_raw_buffer.clone()))?;

    if let types::ui::InputRawBuffer::Index(i) = idx {
        match selected_cursor {
            types::cursor::ConfigPanelCursor::BaudRate => {
                let sel = types::modbus::BaudRateSelector::from_index(i);
                if matches!(sel, types::modbus::BaudRateSelector::Custom { .. }) {
                    // Switch to string input mode for custom baud rate
                    let current_baud = read_status(|status| {
                        Ok(status
                            .ports
                            .map
                            .get(port_name)
                            .map(|port| port.serial_config.baud)
                            .unwrap_or(9600))
                    })?;

                    write_status(|status| {
                        status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::String {
                            bytes: current_baud.to_string().into_bytes(),
                            offset: current_baud.to_string().len() as isize,
                        };
                        Ok(())
                    })?;
                    return Ok(()); // Don't commit yet, wait for string input
                } else {
                    // Update serial config directly
                    write_status(|status| {
                        if let Some(port) = status.ports.map.get_mut(port_name) {
                            port.serial_config.baud = sel.as_u32();
                            port.config_modified = true;
                        }
                        Ok(())
                    })?;
                }
            }
            types::cursor::ConfigPanelCursor::DataBits { .. } => {
                let data_bits = match i {
                    0 => 5,
                    1 => 6,
                    2 => 7,
                    _ => 8,
                };
                write_status(|status| {
                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        port.serial_config.data_bits = data_bits;
                        port.config_modified = true;
                    }
                    Ok(())
                })?;
            }
            types::cursor::ConfigPanelCursor::StopBits => {
                let stop_bits = match i {
                    0 => 1u8,
                    _ => 2u8,
                };
                write_status(|status| {
                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        port.serial_config.stop_bits = stop_bits;
                        port.config_modified = true;
                    }
                    Ok(())
                })?;
            }
            types::cursor::ConfigPanelCursor::Parity => {
                let parity = match i {
                    0 => types::port::SerialParity::None,
                    1 => types::port::SerialParity::Odd,
                    _ => types::port::SerialParity::Even,
                };
                write_status(|status| {
                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        port.serial_config.parity = parity;
                        port.config_modified = true;
                    }
                    Ok(())
                })?;
            }
            types::cursor::ConfigPanelCursor::ProtocolMode => {
                // For now only Modbus RTU option - no action needed
                // Future: When MQTT/TCP support is added, handle protocol switching here
            }
            _ => {}
        }

        write_status(|status| {
            status.temporarily.input_raw_buffer.clear();
            Ok(())
        })?;
    }
    Ok(())
}
