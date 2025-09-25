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
            types::cursor::ModbusDashboardCursor::ModbusMode { index } => {
                // Apply connection mode changes
                let new_mode = if selected_index == 0 {
                    ModbusConnectionMode::Master
                } else {
                    ModbusConnectionMode::Slave
                };

                with_port_write(&port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &mut port.config {
                        let mut all_items: Vec<_> =
                            masters.iter_mut().chain(slaves.iter_mut()).collect();
                        if let Some(item) = all_items.get_mut(index) {
                            item.connection_mode = new_mode;
                            log::info!(
                                "Updated connection mode for index {} to {:?}",
                                index,
                                new_mode
                            );
                        }
                    }
                });
            }
            types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
                // Apply register mode changes
                let new_mode = RegisterMode::from_u8((selected_index as u8) + 1);

                with_port_write(&port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &mut port.config {
                        let mut all_items: Vec<_> =
                            masters.iter_mut().chain(slaves.iter_mut()).collect();
                        if let Some(item) = all_items.get_mut(index) {
                            item.register_mode = new_mode;
                            log::info!(
                                "Updated register mode for index {} to {:?}",
                                index,
                                new_mode
                            );
                        }
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
                        if let types::port::PortConfig::Modbus { masters, slaves } =
                            &mut port.config
                        {
                            let mut all_items: Vec<_> =
                                masters.iter_mut().chain(slaves.iter_mut()).collect();
                            if let Some(item) = all_items.get_mut(index) {
                                item.station_id = station_id;
                                log::info!(
                                    "Updated station ID for index {} to {}",
                                    index,
                                    station_id
                                );
                            }
                        }
                    });
                }
            }
            types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
                if let Ok(start_address) = value.parse::<u16>() {
                    with_port_write(&port, |port| {
                        if let types::port::PortConfig::Modbus { masters, slaves } =
                            &mut port.config
                        {
                            let mut all_items: Vec<_> =
                                masters.iter_mut().chain(slaves.iter_mut()).collect();
                            if let Some(item) = all_items.get_mut(index) {
                                item.register_address = start_address;
                                log::info!(
                                    "Updated register start address for index {} to {}",
                                    index,
                                    start_address
                                );
                            }
                        }
                    });
                }
            }
            types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
                if let Ok(length) = value.parse::<u16>() {
                    with_port_write(&port, |port| {
                        if let types::port::PortConfig::Modbus { masters, slaves } =
                            &mut port.config
                        {
                            let mut all_items: Vec<_> =
                                masters.iter_mut().chain(slaves.iter_mut()).collect();
                            if let Some(item) = all_items.get_mut(index) {
                                item.register_length = length;
                                log::info!(
                                    "Updated register length for index {} to {}",
                                    index,
                                    length
                                );
                            }
                        }
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}
