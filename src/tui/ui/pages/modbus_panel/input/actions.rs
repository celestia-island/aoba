use anyhow::{anyhow, Result};

use crate::{
    protocol::status::{
        read_status,
        types::{self},
        with_port_write, write_status,
    },
    tui::utils::bus::{Bus, UiToCore},
};

pub fn handle_enter_action(bus: &Bus) -> Result<()> {
    let current_cursor = read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            Ok(*cursor)
        } else {
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })?;

    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            create_new_modbus_entry()?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::ModbusMode { index } => {
            // Get the current connection mode value from port config
            let current_value = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    let port_name = format!("COM{}", selected_port + 1);
                    if let Some(port_entry) = status.ports.map.get(&port_name) {
                        if let Ok(port_guard) = port_entry.read() {
                            if let types::port::PortConfig::Modbus { masters, slaves } = &port_guard.config {
                                let mut all_items: Vec<_> = masters.iter().chain(slaves.iter()).collect();
                                if let Some(item) = all_items.get(index) {
                                    return Ok(item.connection_mode as usize);
                                }
                            }
                        }
                    }
                }
                Ok(0) // default to Master
            })?;
            
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::Index(current_value);
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
            // Get the current register mode value from port config  
            let current_value = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    let port_name = format!("COM{}", selected_port + 1);
                    if let Some(port_entry) = status.ports.map.get(&port_name) {
                        if let Ok(port_guard) = port_entry.read() {
                            if let types::port::PortConfig::Modbus { masters, slaves } = &port_guard.config {
                                let mut all_items: Vec<_> = masters.iter().chain(slaves.iter()).collect();
                                if let Some(item) = all_items.get(index) {
                                    return Ok((item.register_mode as u8 - 1) as usize);
                                }
                            }
                        }
                    }
                }
                Ok(2) // default to Holding
            })?;
            
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::Index(current_value);
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::StationId { .. }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { .. }
        | types::cursor::ModbusDashboardCursor::RegisterLength { .. } => {
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::String {
                    bytes: Vec::new(),
                    offset: 0,
                };
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::Register {
            slave_index,
            register_index,
        } => {
            let port_name_opt = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    Ok(status.ports.order.get(*selected_port).cloned())
                } else {
                    Ok(None)
                }
            })?;

            if let Some(port_name) = port_name_opt {
                // Get the register mode to determine behavior
                let register_mode = read_status(|status| {
                    if let Some(port_entry) = status.ports.map.get(&port_name) {
                        if let Ok(port_guard) = port_entry.read() {
                            if let types::port::PortConfig::Modbus { masters, slaves } = &port_guard.config {
                                let mut all_items: Vec<_> = masters.iter().chain(slaves.iter()).collect();
                                if let Some(item) = all_items.get(slave_index) {
                                    return Ok(Some(item.register_mode));
                                }
                            }
                        }
                    }
                    Ok(None)
                })?;

                if let Some(mode) = register_mode {
                    match mode {
                        types::modbus::RegisterMode::Coils | types::modbus::RegisterMode::DiscreteInputs => {
                            // Toggle the coil value directly without entering edit mode
                            with_port_write(&read_status(|status| {
                                Ok(status.ports.map.get(&port_name).cloned())
                            })?.ok_or(anyhow!("Port not found"))?, |port| {
                                if let types::port::PortConfig::Modbus { masters, slaves } = &mut port.config {
                                    let mut all_items: Vec<_> = masters.iter_mut().chain(slaves.iter_mut()).collect();
                                    if let Some(item) = all_items.get_mut(slave_index) {
                                        if let Some(current_value) = item.values.get_mut(register_index) {
                                            *current_value = if *current_value == 0 { 1 } else { 0 };
                                        } else {
                                            // Extend values array if needed
                                            while item.values.len() <= register_index {
                                                item.values.push(0);
                                            }
                                            item.values[register_index] = 1;
                                        }
                                    }
                                }
                            });
                            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
                        }
                        types::modbus::RegisterMode::Holding | types::modbus::RegisterMode::Input => {
                            // Enter edit mode for numeric registers
                            let current_value = read_status(|status| {
                                if let Some(port_entry) = status.ports.map.get(&port_name) {
                                    if let Ok(port_guard) = port_entry.read() {
                                        if let types::port::PortConfig::Modbus { masters, slaves } = &port_guard.config {
                                            let mut all_items: Vec<_> = masters.iter().chain(slaves.iter()).collect();
                                            if let Some(item) = all_items.get(slave_index) {
                                                return Ok(item.values.get(register_index).copied().unwrap_or(0));
                                            }
                                        }
                                    }
                                }
                                Ok(0)
                            })?;
                            
                            let hex_str = format!("0x{current_value:04X}");
                            write_status(|status| {
                                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::String {
                                    bytes: hex_str.clone().into_bytes(),
                                    offset: hex_str.len() as isize,
                                };
                                Ok(())
                            })?;
                            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn handle_leave_page(bus: &Bus) -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;
    write_status(|status| {
        status.page = types::Page::ConfigPanel {
            selected_port,
            view_offset: 0,
            cursor: types::cursor::ConfigPanelCursor::EnablePort,
        };
        Ok(())
    })?;
    bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
    Ok(())
}

fn create_new_modbus_entry() -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    let port_name_opt = read_status(|status| Ok(status.ports.order.get(selected_port).cloned()))?;

    if let Some(port_name) = port_name_opt {
        if let Some(port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
            with_port_write(&port, |port| {
                let types::port::PortConfig::Modbus { masters, .. } = &mut port.config;
                // Create a new master entry with default values
                let new_entry = types::modbus::ModbusRegisterItem {
                    connection_mode: types::modbus::ModbusConnectionMode::Master,
                    station_id: 1,
                    register_mode: types::modbus::RegisterMode::Holding,
                    register_address: 0,
                    register_length: 1,
                    req_success: 0,
                    req_total: 0,
                    next_poll_at: std::time::Instant::now(),
                    pending_requests: Vec::new(),
                    values: Vec::new(),
                };
                masters.push(new_entry);
                log::info!("Created new modbus master entry with station_id=1");
            });
        }
    }
    Ok(())
}
