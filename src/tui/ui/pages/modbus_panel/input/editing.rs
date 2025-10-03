use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{
            self,
            modbus::{ModbusConnectionMode, RegisterMode},
        },
        with_port_write, write_status,
    },
    tui::{
        ui::components::input_span_handler::handle_input_span,
        utils::bus::{Bus, UiToCore},
    },
};

pub fn handle_editing_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Enter => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

            match &input_raw_buffer {
                types::ui::InputRawBuffer::Index(selected_index) => {
                    commit_selector_edit(current_cursor, *selected_index)?;
                }
                types::ui::InputRawBuffer::String { bytes, .. } => {
                    let value = String::from_utf8_lossy(bytes).to_string();
                    commit_text_edit(current_cursor, value)?;
                }
                _ => {}
            }

            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;

            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Esc => {
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;
            if let types::ui::InputRawBuffer::Index(current_index) = input_raw_buffer {
                // Handle selector navigation with proper wrapping
                let current_cursor = read_status(|status| {
                    if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                        Ok(*cursor)
                    } else {
                        Ok(types::cursor::ModbusDashboardCursor::AddLine)
                    }
                })?;

                let max_index = match current_cursor {
                    types::cursor::ModbusDashboardCursor::ModbusMode => 2, // Master, Slave
                    types::cursor::ModbusDashboardCursor::RegisterMode { .. } => 4, // Coils, DiscreteInputs, Holding, Input
                    _ => 0,
                };

                let new_index = if current_index == 0 {
                    max_index - 1 // wrap to last item
                } else {
                    current_index - 1
                };

                write_status(|status| {
                    status.temporarily.input_raw_buffer =
                        types::ui::InputRawBuffer::Index(new_index);
                    Ok(())
                })?;
                bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            } else {
                handle_input_span(key, bus, None, None, |_| true, |_| Ok(()))?;
            }
            Ok(())
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;
            if let types::ui::InputRawBuffer::Index(current_index) = input_raw_buffer {
                // Handle selector navigation with proper wrapping
                let current_cursor = read_status(|status| {
                    if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                        Ok(*cursor)
                    } else {
                        Ok(types::cursor::ModbusDashboardCursor::AddLine)
                    }
                })?;

                let max_index = match current_cursor {
                    types::cursor::ModbusDashboardCursor::ModbusMode => 2, // Master, Slave
                    types::cursor::ModbusDashboardCursor::RegisterMode { .. } => 4, // Coils, DiscreteInputs, Holding, Input
                    _ => 0,
                };

                let new_index = if current_index + 1 >= max_index {
                    0 // wrap to first item
                } else {
                    current_index + 1
                };

                write_status(|status| {
                    status.temporarily.input_raw_buffer =
                        types::ui::InputRawBuffer::Index(new_index);
                    Ok(())
                })?;
                bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            } else {
                handle_input_span(key, bus, None, None, |_| true, |_| Ok(()))?;
            }
            Ok(())
        }
        _ => {
            handle_input_span(key, bus, None, None, |_| true, |_| Ok(()))?;
            Ok(())
        }
    }
}

fn commit_selector_edit(
    cursor: types::cursor::ModbusDashboardCursor,
    selected_index: usize,
) -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    let port_name = format!("COM{}", selected_port + 1);

    if let Some(port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
        match cursor {
            types::cursor::ModbusDashboardCursor::ModbusMode => {
                // Apply global mode changes to all stations in this port
                let new_mode = if selected_index == 0 {
                    ModbusConnectionMode::default_master()
                } else {
                    ModbusConnectionMode::default_slave()
                };

                with_port_write(&port, |port| {
                    let types::port::PortConfig::Modbus { mode, stations } = &mut port.config;
                    *mode = new_mode.clone();
                    // Update all existing stations to match the new global mode
                    for station in stations.iter_mut() {
                        station.connection_mode = new_mode.clone();
                    }
                    log::info!("Updated global connection mode to {:?}", mode.is_master());
                });
            }
            types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
                // Apply register mode changes
                let new_mode = RegisterMode::from_u8((selected_index as u8) + 1);

                with_port_write(&port, |port| {
                    let types::port::PortConfig::Modbus { mode: _, stations } = &mut port.config;
                    let mut all_items: Vec<_> = stations.iter_mut().collect();
                    if let Some(item) = all_items.get_mut(index) {
                        item.register_mode = new_mode;
                        log::info!("Updated register mode for index {index} to {new_mode:?}");
                    }
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn commit_text_edit(cursor: types::cursor::ModbusDashboardCursor, value: String) -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    let port_name = format!("COM{}", selected_port + 1);

    if let Some(port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
        match cursor {
            types::cursor::ModbusDashboardCursor::StationId { index } => {
                if let Ok(station_id) = value.parse::<u8>() {
                    with_port_write(&port, |port| {
                        let types::port::PortConfig::Modbus { mode: _, stations } =
                            &mut port.config;
                        let mut all_items: Vec<_> = stations.iter_mut().collect();
                        if let Some(item) = all_items.get_mut(index) {
                            item.station_id = station_id;
                            log::info!("Updated station ID for index {index} to {station_id}");
                        }
                    });
                }
            }
            types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
                if let Ok(start_address) = value.parse::<u16>() {
                    with_port_write(&port, |port| {
                        let types::port::PortConfig::Modbus { mode: _, stations } =
                            &mut port.config;
                        let mut all_items: Vec<_> = stations.iter_mut().collect();
                        if let Some(item) = all_items.get_mut(index) {
                            item.register_address = start_address;
                            log::info!("Updated register start address for index {index} to {start_address}");
                        }
                    });
                }
            }
            types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
                if let Ok(length) = value.parse::<u16>() {
                    with_port_write(&port, |port| {
                        let types::port::PortConfig::Modbus { mode: _, stations } =
                            &mut port.config;
                        let mut all_items: Vec<_> = stations.iter_mut().collect();
                        if let Some(item) = all_items.get_mut(index) {
                            item.register_length = length;
                            log::info!("Updated register length for index {index} to {length}");
                        }
                    });
                }
            }
            types::cursor::ModbusDashboardCursor::Register {
                slave_index,
                register_index,
            } => {
                // Parse hex value, supporting both 0x prefix and plain hex
                let parsed_value = if value.starts_with("0x") || value.starts_with("0X") {
                    u16::from_str_radix(&value[2..], 16)
                } else if value.is_empty() {
                    Ok(0) // Empty input defaults to 0
                } else {
                    u16::from_str_radix(&value, 16)
                };

                if let Ok(register_value) = parsed_value {
                    with_port_write(&port, |port| {
                        let types::port::PortConfig::Modbus { mode: _, stations } =
                            &mut port.config;
                        let mut all_items: Vec<_> = stations.iter_mut().collect();
                        if let Some(item) = all_items.get_mut(slave_index) {
                            // TODO: Update global storage when mode is Master
                            log::info!(
                                "Updated register value for slave {} register {} to 0x{:04X}",
                                item.station_id,
                                register_index,
                                register_value
                            );
                        }
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}
