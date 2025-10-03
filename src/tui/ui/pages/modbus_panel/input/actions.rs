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
    log::info!("handle_enter_action: current_cursor={current_cursor:?}");

    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            create_new_modbus_entry()?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::ModbusMode => {
            // Toggle global mode for this port between Master and Slave
            let current_mode = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    log::info!("ModbusMode Enter: selected_port={selected_port}");
                    if let Some(port_name) = status.ports.order.get(*selected_port) {
                        log::info!("ModbusMode Enter: port_name={port_name}");
                        if let Some(port_entry) = status.ports.map.get(port_name) {
                            if let Ok(port_guard) = port_entry.read() {
                                let types::port::PortConfig::Modbus { mode, stations: _ } =
                                    &port_guard.config;
                                let mode_index = if mode.is_master() { 0 } else { 1 };
                                log::info!("ModbusMode Enter: current mode index={mode_index}");
                                return Ok(mode_index);
                            } else {
                                log::warn!("ModbusMode Enter: Failed to acquire read lock");
                            }
                        } else {
                            log::warn!("ModbusMode Enter: Port not found in map");
                        }
                    } else {
                        log::warn!(
                            "ModbusMode Enter: Port not found in order at index {selected_port}"
                        );
                    }
                }
                log::info!("ModbusMode Enter: Falling back to default Master mode");
                Ok(0) // default to Master
            })?;

            write_status(|status| {
                status.temporarily.input_raw_buffer =
                    types::ui::InputRawBuffer::Index(current_mode);
                log::info!(
                    "ModbusMode Enter: Set input_raw_buffer to Index({current_mode})"
                );
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
            // Get the current register mode value from port config
            let current_value = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    if let Some(port_name) = status.ports.order.get(*selected_port) {
                        if let Some(port_entry) = status.ports.map.get(port_name) {
                            if let Ok(port_guard) = port_entry.read() {
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port_guard.config;
                                let all_items: Vec<_> = stations.iter().collect();
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
                status.temporarily.input_raw_buffer =
                    types::ui::InputRawBuffer::Index(current_value);
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
            let port_name_opt = read_status(|status| match &status.page {
                types::Page::ModbusDashboard { selected_port, .. } => {
                    Ok(status.ports.order.get(*selected_port).cloned())
                }
                _ => Ok(None),
            })?;

            if let Some(port_name) = port_name_opt {
                // Get the register mode to determine behavior
                let register_mode = read_status(|status| {
                    if let Some(port_entry) = status.ports.map.get(&port_name) {
                        if let Ok(port_guard) = port_entry.read() {
                            let types::port::PortConfig::Modbus { mode: _, stations } =
                                &port_guard.config;
                            let all_items: Vec<_> = stations.iter().collect();
                            if let Some(item) = all_items.get(slave_index) {
                                return Ok(Some(item.register_mode));
                            }
                        }
                    }
                    Ok(None)
                })?;

                if let Some(mode) = register_mode {
                    match mode {
                        types::modbus::RegisterMode::Coils
                        | types::modbus::RegisterMode::DiscreteInputs => {
                            // Toggle the coil value directly without entering edit mode
                            with_port_write(
                                &read_status(|status| {
                                    Ok(status.ports.map.get(&port_name).cloned())
                                })?
                                .ok_or(anyhow!("Port not found"))?,
                                |port| {
                                    let types::port::PortConfig::Modbus { mode: _, stations } =
                                        &mut port.config;
                                    let mut all_items: Vec<_> = stations.iter_mut().collect();
                                    if let Some(item) = all_items.get_mut(slave_index) {
                                        // TODO: Update global storage when mode is Master
                                        // For now, just log the action
                                        log::info!(
                                            "Toggle register {} for station {}",
                                            register_index,
                                            item.station_id
                                        );
                                    }
                                },
                            );
                            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
                        }
                        types::modbus::RegisterMode::Holding
                        | types::modbus::RegisterMode::Input => {
                            // Enter edit mode for numeric registers
                            let current_value = read_status(|status| {
                                if let Some(port_entry) = status.ports.map.get(&port_name) {
                                    if let Ok(port_guard) = port_entry.read() {
                                        let types::port::PortConfig::Modbus { mode: _, stations } =
                                            &port_guard.config;
                                        let all_items: Vec<_> = stations.iter().collect();
                                        if let Some(_item) = all_items.get(slave_index) {
                                            // TODO: Read from global storage when mode is Master
                                            return Ok(0); // Placeholder value
                                        }
                                    }
                                }
                                Ok(0)
                            })?;

                            // Format hex string and strip leading zeros
                            let hex_str = if current_value == 0 {
                                "0".to_string()
                            } else {
                                format!("{current_value:X}") // No leading zeros, uppercase hex
                            };

                            write_status(|status| {
                                status.temporarily.input_raw_buffer =
                                    types::ui::InputRawBuffer::String {
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
                let types::port::PortConfig::Modbus { mode, stations } = &mut port.config;
                // Create a new entry with the global mode from the port config
                let new_entry = types::modbus::ModbusRegisterItem {
                    connection_mode: mode.clone(),
                    station_id: 1,
                    register_mode: types::modbus::RegisterMode::Holding,
                    register_address: 0,
                    register_length: 1,
                    req_success: 0,
                    req_total: 0,
                    next_poll_at: std::time::Instant::now() - std::time::Duration::from_secs(1), // Start immediately
                    last_request_time: None,
                    pending_requests: Vec::new(),
                };
                stations.push(new_entry);
                log::info!(
                    "Created new modbus entry with station_id=1 in {:?} mode",
                    if mode.is_master() { "Master" } else { "Slave" }
                );
            });
        }
    }
    Ok(())
}
