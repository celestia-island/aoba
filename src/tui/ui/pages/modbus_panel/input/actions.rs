use anyhow::{anyhow, Result};

use crate::{
    protocol::status::{
        read_status,
        types::{
            self,
            port::{PortOwner, PortState, PortSubprocessMode},
        },
        with_port_write, write_status,
    },
    tui::utils::bus::{Bus, UiToCore},
};

pub fn handle_enter_action(bus: &Bus) -> Result<()> {
    log::info!("🔵 handle_enter_action called");
    let current_cursor = read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            log::info!("🔵 Current cursor in ModbusDashboard: {cursor:?}");
            Ok(*cursor)
        } else {
            log::warn!("🔵 Not in ModbusDashboard page, using default AddLine cursor");
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })?;

    log::info!("🔵 Processing cursor action for: {current_cursor:?}");
    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            log::info!("🔵 AddLine action - calling create_new_modbus_entry");
            create_new_modbus_entry(bus)?;
            log::info!("🔵 Station created successfully, sending refresh");
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            log::info!("🔵 Refresh sent");
        }
        types::cursor::ModbusDashboardCursor::ModbusMode => {
            // Toggle global mode for this port between Master and Slave
            let current_mode = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    if let Some(port_name) = status.ports.order.get(*selected_port) {
                        if let Some(port_entry) = status.ports.map.get(port_name) {
                            if let Ok(port_guard) = port_entry.read() {
                                let types::port::PortConfig::Modbus { mode, stations: _ } =
                                    &port_guard.config;
                                return Ok(if mode.is_master() { 0 } else { 1 });
                            }
                        }
                    }
                }
                Ok(0) // default to Master
            })?;

            write_status(|status| {
                status.temporarily.input_raw_buffer =
                    types::ui::InputRawBuffer::Index(current_mode);
                Ok(())
            })?;
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
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
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
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
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
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
                            // Toggle coil value
                            let selected_port = read_status(|status| {
                                if let types::Page::ModbusDashboard { selected_port, .. } =
                                    &status.page
                                {
                                    Ok(*selected_port)
                                } else {
                                    Ok(0)
                                }
                            })?;

                            let port_name_opt = read_status(|status| {
                                Ok(status.ports.order.get(selected_port).cloned())
                            })?;

                            if let Some(port_name) = port_name_opt {
                                if let Some(port) = read_status(|status| {
                                    Ok(status.ports.map.get(&port_name).cloned())
                                })? {
                                    let mut owner_snapshot: Option<PortOwner> = None;
                                    let mut register_update: Option<(String, u8, u16, Vec<u16>)> =
                                        None;

                                    with_port_write(&port, |port| {
                                        owner_snapshot = port.state.owner().cloned();

                                        let types::port::PortConfig::Modbus { mode, stations } =
                                            &mut port.config;
                                        if let Some(item) = stations.get_mut(slave_index) {
                                            let register_addr =
                                                item.register_address + register_index as u16;

                                            let value_index =
                                                (register_addr - item.register_address) as usize;
                                            if item.last_values.len() <= value_index {
                                                item.last_values.resize(value_index + 1, 0);
                                            }

                                            let current = item.last_values[value_index] != 0;
                                            let new_value_flag = !current;
                                            item.last_values[value_index] =
                                                if new_value_flag { 1 } else { 0 };

                                            match mode {
                                                types::modbus::ModbusConnectionMode::Master => {
                                                    register_update = Some((
                                                        "coil".to_string(),
                                                        item.station_id,
                                                        register_addr,
                                                        vec![item.last_values[value_index]],
                                                    ));
                                                }
                                                types::modbus::ModbusConnectionMode::Slave {
                                                    ..
                                                } => {
                                                    let should_queue = match owner_snapshot.as_ref() {
                                                        Some(PortOwner::CliSubprocess(info))
                                                            if info.mode == PortSubprocessMode::MasterProvide =>
                                                        {
                                                            false
                                                        }
                                                        _ => true,
                                                    };

                                                    if should_queue {
                                                        use crate::protocol::modbus::generate_pull_set_holding_request;

                                                        let coil_value = if new_value_flag {
                                                            0xFF00
                                                        } else {
                                                            0x0000
                                                        };

                                                        if let Ok((_request, raw_frame)) =
                                                            generate_pull_set_holding_request(
                                                                item.station_id,
                                                                register_addr,
                                                                coil_value,
                                                            )
                                                        {
                                                            let mut frame = raw_frame;
                                                            if frame.len() > 1 {
                                                                frame[1] = 0x05;
                                                                // single coil write
                                                            }
                                                            item.pending_requests
                                                                .extend_from_slice(&frame);
                                                            log::info!(
                                                                "📤 Slave: Queued coil toggle 0x{:04X} -> {} ({} bytes)",
                                                                register_addr,
                                                                new_value_flag,
                                                                frame.len()
                                                            );
                                                        } else {
                                                            log::warn!(
                                                                "Failed to build single-coil write frame for station {}",
                                                                item.station_id
                                                            );
                                                        }
                                                    }

                                                    register_update = Some((
                                                        "coil".to_string(),
                                                        item.station_id,
                                                        register_addr,
                                                        vec![item.last_values[value_index]],
                                                    ));
                                                }
                                            }
                                        }
                                    });

                                    if let (Some(PortOwner::CliSubprocess(_)), Some(update)) =
                                        (owner_snapshot, register_update)
                                    {
                                        if let Err(err) =
                                            bus.ui_tx.send(UiToCore::SendRegisterUpdate {
                                                port_name: port_name.clone(),
                                                station_id: update.1,
                                                register_type: update.0,
                                                start_address: update.2,
                                                values: update.3,
                                            })
                                        {
                                            log::warn!(
                                                "Failed to send coil toggle IPC message: {err}"
                                            );
                                        }
                                    }
                                }
                            }
                            bus.ui_tx
                                .send(UiToCore::Refresh)
                                .map_err(|err| anyhow!(err))?;
                        }
                        types::modbus::RegisterMode::Holding
                        | types::modbus::RegisterMode::Input => {
                            // Enter edit mode for numeric registers with empty buffer
                            // (User will type the new value from scratch)
                            write_status(|status| {
                                status.temporarily.input_raw_buffer =
                                    types::ui::InputRawBuffer::String {
                                        bytes: Vec::new(),
                                        offset: 0,
                                    };
                                Ok(())
                            })?;
                            bus.ui_tx
                                .send(UiToCore::Refresh)
                                .map_err(|err| anyhow!(err))?;
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
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

fn create_new_modbus_entry(bus: &Bus) -> Result<()> {
    log::info!("🟢 create_new_modbus_entry called");
    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            log::info!("🟢 Selected port index: {selected_port}");
            Ok(*selected_port)
        } else {
            log::warn!("🟢 Not in ModbusDashboard page, using default port 0");
            Ok(0)
        }
    })?;

    let port_name_opt = read_status(|status| {
        let name = status.ports.order.get(selected_port).cloned();
        log::info!("🟢 Port name at index {selected_port}: {name:?}");
        Ok(name)
    })?;

    if let Some(port_name) = port_name_opt {
        log::info!("🟢 Found port name: {port_name}");
        let mut should_restart_runtime = false;
        if let Some(port) = read_status(|status| {
            let port = status.ports.map.get(&port_name).cloned();
            if port.is_some() {
                log::info!("🟢 Port entry found in map for: {port_name}");
            } else {
                log::warn!("🟢 Port entry NOT found in map for: {port_name}");
            }
            Ok(port)
        })? {
            log::info!("🟢 Calling with_port_write for: {port_name}");
            with_port_write(&port, |port| {
                log::info!("🟢 Inside with_port_write closure");
                // Check if port is currently occupied before adding station
                if matches!(
                    port.state,
                    PortState::OccupiedByThis {
                        owner: PortOwner::Runtime(_)
                    }
                ) {
                    log::info!(
                        "🟢 Port {port_name} is occupied by native runtime - scheduling restart"
                    );
                    should_restart_runtime = true;
                }

                let types::port::PortConfig::Modbus { mode, stations } = &mut port.config;
                log::info!(
                    "🟢 Current mode: {:?}, current stations count: {}",
                    if mode.is_master() { "Master" } else { "Slave" },
                    stations.len()
                );
                // Create a new entry with the global mode from the port config
                let new_entry = types::modbus::ModbusRegisterItem {
                    station_id: 1,
                    register_mode: types::modbus::RegisterMode::Holding,
                    register_address: 0,
                    register_length: 1,
                    last_values: vec![0],
                    req_success: 0,
                    req_total: 0,
                    next_poll_at: std::time::Instant::now() - std::time::Duration::from_secs(1), // Start immediately
                    last_request_time: None,
                    last_response_time: None,
                    pending_requests: Vec::new(),
                };
                log::info!("🟢 Pushing new station entry");
                stations.push(new_entry);
                log::info!(
                    "✅ Created new modbus entry with station_id=1 in {:?} mode (total stations now: {})",
                    if mode.is_master() { "Master" } else { "Slave" },
                    stations.len()
                );
            });
            log::info!("🟢 with_port_write completed");

            // If port was occupied, restart it to apply new station configuration
            if should_restart_runtime {
                log::info!(
                    "🔄 Restarting native runtime for {port_name} to apply new station configuration"
                );
                bus.ui_tx
                    .send(UiToCore::ToggleRuntime(port_name.clone()))
                    .map_err(|err| anyhow!("Failed to send ToggleRuntime for restart: {err}"))?;
                bus.ui_tx
                    .send(UiToCore::ToggleRuntime(port_name.clone()))
                    .map_err(|err| {
                        anyhow!("Failed to send ToggleRuntime for restart (start phase): {err}")
                    })?;
            }
        } else {
            log::error!("🟢 Port entry is None for: {port_name}");
        }
    } else {
        log::error!("🟢 Port name is None at selected_port index: {selected_port}");
    }
    log::info!("🟢 create_new_modbus_entry completed");
    Ok(())
}
