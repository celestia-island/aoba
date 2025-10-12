use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use rmodbus::server::context::ModbusContext;

use crate::{
    protocol::status::{
        read_status,
        types::{
            self,
            modbus::{ModbusConnectionMode, RegisterMode},
            port::{PortOwner, PortState, PortSubprocessMode},
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
            let buffer_type = match &input_raw_buffer {
                types::ui::InputRawBuffer::None => "None".to_string(),
                types::ui::InputRawBuffer::Index(i) => format!("Index({})", i),
                types::ui::InputRawBuffer::String { bytes, .. } => {
                    format!(
                        "String(len={}, val='{}')",
                        bytes.len(),
                        String::from_utf8_lossy(bytes)
                    )
                }
            };
            log::info!("🟡 handle_editing_input: buffer type = {}", buffer_type);

            let mut maybe_restart: Option<String> = None;

            match &input_raw_buffer {
                types::ui::InputRawBuffer::Index(selected_index) => {
                    log::info!("🟡 Committing selector edit, index={}", selected_index);
                    maybe_restart = commit_selector_edit(current_cursor, *selected_index)?;
                }
                types::ui::InputRawBuffer::String { bytes, .. } => {
                    let value = String::from_utf8_lossy(bytes).to_string();
                    log::info!("🟡 Committing text edit, value='{}'", value);
                    commit_text_edit(current_cursor, value, bus)?;
                }
                _ => {
                    log::warn!("🟡 Buffer is None, skipping commit");
                }
            }

            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;

            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;

            if let Some(port_name) = maybe_restart {
                bus.ui_tx
                    .send(UiToCore::ToggleRuntime(port_name))
                    .map_err(|err| anyhow!(err))?;
            }
            Ok(())
        }
        KeyCode::Esc => {
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;
            bus.ui_tx
                .send(UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
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
                bus.ui_tx
                    .send(UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
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
                bus.ui_tx
                    .send(UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
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
) -> Result<Option<String>> {
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
            match cursor {
                types::cursor::ModbusDashboardCursor::ModbusMode => {
                    // Apply global mode changes to all stations in this port
                    let new_mode = if selected_index == 0 {
                        ModbusConnectionMode::default_master()
                    } else {
                        ModbusConnectionMode::default_slave()
                    };

                    let mut should_restart = false;
                    with_port_write(&port, |port| {
                        // evaluate occupancy before taking a mutable borrow of port.config
                        let was_occupied_by_this =
                            matches!(port.state, PortState::OccupiedByThis { .. });

                        let types::port::PortConfig::Modbus { mode, stations } = &mut port.config;
                        let old_was_master = mode.is_master();
                        let new_is_master = new_mode.is_master();

                        if old_was_master != new_is_master && was_occupied_by_this {
                            should_restart = true;
                        }

                        *mode = new_mode.clone();
                        // Update all existing stations to match the new global mode
                        for station in stations.iter_mut() {
                            station.connection_mode = new_mode.clone();
                        }
                        log::info!("Updated global connection mode to {:?}", mode.is_master());
                    });

                    if should_restart {
                        return Ok(Some(port_name.clone()));
                    }
                }
                types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
                    // Apply register mode changes
                    let new_mode = RegisterMode::from_u8((selected_index as u8) + 1);

                    with_port_write(&port, |port| {
                        let types::port::PortConfig::Modbus { mode: _, stations } =
                            &mut port.config;
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
    }
    Ok(None)
}

fn commit_text_edit(
    cursor: types::cursor::ModbusDashboardCursor,
    value: String,
    bus: &Bus,
) -> Result<()> {
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
                        // Get selected port and port name
                        let selected_port = read_status(|status| {
                            if let types::Page::ModbusDashboard { selected_port, .. } = &status.page
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
                            if let Some(port) =
                                read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                            {
                                let mut cli_data_update: Option<(
                                    String,
                                    Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
                                    u16,
                                    u16,
                                    RegisterMode,
                                )> = None;

                                with_port_write(&port, |port| {
                                    let owner_info = port.state.owner().cloned();
                                    log::info!(
                                        "🔍 commit_text_edit: owner_info = {:?}",
                                        owner_info
                                    );
                                    let types::port::PortConfig::Modbus { mode, stations } =
                                        &mut port.config;

                                    if let Some(item) = stations.get_mut(slave_index) {
                                        let register_addr =
                                            item.register_address + register_index as u16;

                                        match mode {
                                            ModbusConnectionMode::Master { storage } => {
                                                if let Ok(mut context) = storage.lock() {
                                                    match item.register_mode {
                                                        RegisterMode::Holding => {
                                                            if let Err(err) = context.set_holding(
                                                                register_addr,
                                                                register_value,
                                                            ) {
                                                                log::warn!(
                                                                    "Failed to set holding register at {register_addr}: {err}"
                                                                );
                                                            } else {
                                                                log::info!(
                                                                    "✓ Master: Set holding register at 0x{register_addr:04X} = 0x{register_value:04X}"
                                                                );

                                                                // Send IPC update if using CLI subprocess
                                                                log::info!("🔍 Checking if we should send IPC update, owner_info: {:?}", owner_info.as_ref().map(|o| match o {
                                                                    PortOwner::Runtime(_) => "Runtime",
                                                                    PortOwner::CliSubprocess(_) => "CliSubprocess"
                                                                }));
                                                                if let Some(
                                                                    PortOwner::CliSubprocess(_),
                                                                ) = owner_info.as_ref()
                                                                {
                                                                    log::info!("🔍 Sending IPC RegisterUpdate message");
                                                                    // Send register update via IPC for real-time synchronization
                                                                    if let Err(err) = bus.ui_tx.send(UiToCore::SendRegisterUpdate {
                                                                        port_name: port_name.clone(),
                                                                        station_id: item.station_id,
                                                                        register_type: "holding".to_string(),
                                                                        start_address: register_addr,
                                                                        values: vec![register_value],
                                                                    }) {
                                                                        log::warn!("Failed to send IPC register update message: {err}");
                                                                    } else {
                                                                        log::info!("✅ IPC RegisterUpdate message sent successfully");
                                                                    }
                                                                } else {
                                                                    log::warn!("🔍 NOT sending IPC because owner is not CliSubprocess");
                                                                }
                                                            }
                                                        }
                                                        RegisterMode::Coils => {
                                                            let coil_value = register_value != 0;
                                                            if let Err(err) = context
                                                                .set_coil(register_addr, coil_value)
                                                            {
                                                                log::warn!(
                                                                    "Failed to set coil at {register_addr}: {err}"
                                                                );
                                                            } else {
                                                                log::info!(
                                                                    "✓ Master: Set coil at 0x{register_addr:04X} = {coil_value}"
                                                                );

                                                                // Send IPC update if using CLI subprocess
                                                                if let Some(
                                                                    PortOwner::CliSubprocess(_),
                                                                ) = owner_info.as_ref()
                                                                {
                                                                    // Send register update via IPC for real-time synchronization
                                                                    // For coils, send as 0/1 values
                                                                    if let Err(err) = bus.ui_tx.send(UiToCore::SendRegisterUpdate {
                                                                        port_name: port_name.clone(),
                                                                        station_id: item.station_id,
                                                                        register_type: "coil".to_string(),
                                                                        start_address: register_addr,
                                                                        values: vec![if coil_value { 1 } else { 0 }],
                                                                    }) {
                                                                        log::warn!("Failed to send IPC register update message: {err}");
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        _ => {
                                                            log::warn!(
                                                                "Cannot write to read-only register type: {:?}",
                                                                item.register_mode
                                                            );
                                                        }
                                                    }
                                                }

                                                // If using CLI subprocess, also update the data source file
                                                log::info!(
                                                    "🔍 Master mode: owner_info = {:?}",
                                                    owner_info
                                                );
                                                if let Some(PortOwner::CliSubprocess(info)) =
                                                    owner_info.clone()
                                                {
                                                    log::info!("🔍 Master mode: Checking if subprocess mode is MasterProvide: {:?}", info.mode);
                                                    if info.mode
                                                        == PortSubprocessMode::MasterProvide
                                                    {
                                                        if let Some(path) =
                                                            info.data_source_path.clone()
                                                        {
                                                            log::info!("🔍 Master mode: Setting cli_data_update for path: {}", path);
                                                            cli_data_update = Some((
                                                                path,
                                                                Arc::clone(storage),
                                                                item.register_address,
                                                                item.register_length,
                                                                item.register_mode,
                                                            ));
                                                        } else {
                                                            log::warn!(
                                                                "CLI subprocess missing data source path for {port_name}"
                                                            );
                                                        }
                                                    } else {
                                                        log::warn!("🔍 Master mode: subprocess mode is NOT MasterProvide: {:?}", info.mode);
                                                    }
                                                } else {
                                                    log::warn!("🔍 Master mode: owner_info is NOT CliSubprocess");
                                                }
                                            }
                                            ModbusConnectionMode::Slave { storage, .. } => {
                                                if let Some(PortOwner::CliSubprocess(info)) =
                                                    owner_info.clone()
                                                {
                                                    if info.mode
                                                        == PortSubprocessMode::MasterProvide
                                                    {
                                                        if let Some(path) =
                                                            info.data_source_path.clone()
                                                        {
                                                            if let Ok(mut context) = storage.lock()
                                                            {
                                                                match item.register_mode {
                                                                    RegisterMode::Holding => {
                                                                        if let Err(err) = context
                                                                            .set_holding(
                                                                                register_addr,
                                                                                register_value,
                                                                            )
                                                                        {
                                                                            log::warn!(
                                                                                "Failed to set holding register at {register_addr}: {err}"
                                                                            );
                                                                        }
                                                                    }
                                                                    RegisterMode::Coils => {
                                                                        let coil_value =
                                                                            register_value != 0;
                                                                        if let Err(err) = context
                                                                            .set_coil(
                                                                                register_addr,
                                                                                coil_value,
                                                                            )
                                                                        {
                                                                            log::warn!(
                                                                                "Failed to set coil at {register_addr}: {err}"
                                                                            );
                                                                        }
                                                                    }
                                                                    _ => {
                                                                        log::warn!(
                                                                            "Cannot write to read-only register type: {:?}",
                                                                            item.register_mode
                                                                        );
                                                                    }
                                                                }
                                                            }

                                                            cli_data_update = Some((
                                                                path,
                                                                Arc::clone(storage),
                                                                item.register_address,
                                                                item.register_length,
                                                                item.register_mode,
                                                            ));
                                                        } else {
                                                            log::warn!(
                                                                "CLI subprocess missing data source path for {port_name}"
                                                            );
                                                        }
                                                    } else {
                                                        enqueue_slave_write(
                                                            item,
                                                            register_addr,
                                                            register_value,
                                                        );
                                                    }
                                                } else {
                                                    enqueue_slave_write(
                                                        item,
                                                        register_addr,
                                                        register_value,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                });

                                if let Some((path, storage, base_addr, length, reg_mode)) =
                                    cli_data_update
                                {
                                    let path_buf = std::path::PathBuf::from(path);
                                    // For TUI register updates, replace the file to keep only latest state
                                    // This prevents file from accumulating history across multiple update cycles
                                    if let Err(err) = crate::tui::replace_cli_data_snapshot(
                                        &path_buf, &storage, base_addr, length, reg_mode,
                                    ) {
                                        log::warn!(
                                            "Failed to replace CLI data snapshot for {port_name}: {err}"
                                        );
                                    } else {
                                        log::info!(
                                            "CLI[{port_name}]: replaced data snapshot addr=0x{base_addr:04X} len={length} mode={reg_mode:?}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn enqueue_slave_write(
    item: &mut types::modbus::ModbusRegisterItem,
    register_addr: u16,
    register_value: u16,
) {
    use crate::protocol::modbus::generate_pull_set_holding_request;

    match item.register_mode {
        RegisterMode::Holding => {
            if let Ok((_request, raw_frame)) =
                generate_pull_set_holding_request(item.station_id, register_addr, register_value)
            {
                item.pending_requests.extend_from_slice(&raw_frame);
                log::info!(
                    "📤 Slave: Queued write request for holding register 0x{:04X} = 0x{:04X} ({} bytes)",
                    register_addr,
                    register_value,
                    raw_frame.len()
                );
            } else {
                log::warn!("Failed to generate write request for holding register");
            }
        }
        RegisterMode::Coils => {
            log::info!(
                "📤 Slave: Coil write request for 0x{:04X} = {} (coil writes need set_coils_bulk implementation)",
                register_addr,
                register_value != 0
            );
        }
        _ => {
            log::warn!(
                "Cannot write to read-only register type: {:?}",
                item.register_mode
            );
        }
    }
}
