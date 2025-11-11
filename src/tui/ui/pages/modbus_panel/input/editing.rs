use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    i18n::lang,
    protocol::modbus::generate_pull_set_holding_request,
    tui::{
        status as types,
        status::{
            modbus::{
                ModbusConnectionMode, ModbusMasterDataSource, ModbusMasterDataSourceKind,
                RegisterMode, StationMode,
            },
            port::{PortState, PortSubprocessInfo, PortSubprocessMode},
            {read_status, write_status},
        },
        ui::components::input_span_handler::handle_input_span,
        utils::bus::{self, Bus, UiToCore},
    },
};

pub fn handle_editing_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Enter => {
            let current_cursor = read_status(|status| {
                if let crate::tui::status::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;
            let buffer_type = match &input_raw_buffer {
                types::ui::InputRawBuffer::None => "None".to_string(),
                types::ui::InputRawBuffer::Index(i) => format!("Index({i})"),
                types::ui::InputRawBuffer::String { bytes, .. } => {
                    format!(
                        "String(len={}, val='{}')",
                        bytes.len(),
                        String::from_utf8_lossy(bytes)
                    )
                }
            };
            log::info!("🟡 handle_editing_input: buffer type = {buffer_type}");

            let mut maybe_restart: Option<String> = None;

            match &input_raw_buffer {
                types::ui::InputRawBuffer::Index(selected_index) => {
                    log::info!("🟡 Committing selector edit, index={selected_index}");
                    maybe_restart = commit_selector_edit(current_cursor, *selected_index)?;
                }
                types::ui::InputRawBuffer::String { bytes, .. } => {
                    let value = String::from_utf8_lossy(bytes).to_string();
                    log::info!("🟡 Committing text edit, value='{value}'");
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

            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;

            if let Some(port_name) = maybe_restart {
                bus.ui_tx
                    .send(UiToCore::RestartRuntime(port_name))
                    .map_err(|err| anyhow!(err))?;
            }
            Ok(())
        }
        KeyCode::Esc => {
            // Check if Ctrl is pressed for "force return without saving"
            let is_ctrl_esc = key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL);

            if is_ctrl_esc {
                // Ctrl+Esc: Force return without saving
                log::info!("⚠️ Ctrl+Esc: Discarding changes and returning");
                write_status(|status| {
                    status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                    Ok(())
                })?;
                bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
                return Ok(());
            }

            // Regular Esc: Save changes if any, then return
            let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

            // Check if there's pending data to save
            let has_pending_data = !matches!(input_raw_buffer, types::ui::InputRawBuffer::None);

            if has_pending_data {
                log::info!("💾 Esc: Saving pending changes before returning");

                let current_cursor = read_status(|status| {
                    if let crate::tui::status::Page::ModbusDashboard { cursor, .. } = &status.page {
                        Ok(*cursor)
                    } else {
                        Ok(types::cursor::ModbusDashboardCursor::AddLine)
                    }
                })?;

                let mut maybe_restart: Option<String> = None;

                match &input_raw_buffer {
                    types::ui::InputRawBuffer::Index(selected_index) => {
                        log::info!("💾 Committing selector edit on Esc, index={selected_index}");
                        maybe_restart = commit_selector_edit(current_cursor, *selected_index)?;
                    }
                    types::ui::InputRawBuffer::String { bytes, .. } => {
                        let value = String::from_utf8_lossy(bytes).to_string();
                        log::info!("💾 Committing text edit on Esc, value='{value}'");
                        commit_text_edit(current_cursor, value, bus)?;
                    }
                    _ => {}
                }

                write_status(|status| {
                    status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                    Ok(())
                })?;

                bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;

                if let Some(port_name) = maybe_restart {
                    bus.ui_tx
                        .send(UiToCore::RestartRuntime(port_name))
                        .map_err(|err| anyhow!(err))?;
                }
            } else {
                // No pending data, just clear buffer and refresh
                log::info!("↩️ Esc: No pending changes, just exiting edit mode");
                write_status(|status| {
                    status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                    Ok(())
                })?;
                bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
            }

            Ok(())
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;
            if let types::ui::InputRawBuffer::Index(current_index) = input_raw_buffer {
                // Handle selector navigation with proper wrapping
                let current_cursor = read_status(|status| {
                    if let crate::tui::status::Page::ModbusDashboard { cursor, .. } = &status.page {
                        Ok(*cursor)
                    } else {
                        Ok(types::cursor::ModbusDashboardCursor::AddLine)
                    }
                })?;

                let max_index = match current_cursor {
                    types::cursor::ModbusDashboardCursor::ModbusMode => 2, // Master, Slave
                    types::cursor::ModbusDashboardCursor::MasterSourceKind => {
                        types::modbus::ModbusMasterDataSourceKind::all().len()
                    }
                    types::cursor::ModbusDashboardCursor::MasterSourceValue => {
                        // Count available ports for TransparentForward selector
                        read_status(|status| {
                            if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
                                if let Some(port_name) = status.ports.order.get(*selected_port) {
                                    // Count ports excluding the current one
                                    let count = status.ports.order.iter()
                                        .filter(|p| *p != port_name)
                                        .count();
                                    return Ok(count);
                                }
                            }
                            Ok(0)
                        }).unwrap_or(0)
                    }
                    types::cursor::ModbusDashboardCursor::RegisterMode { .. } => 4, // Coils, DiscreteInputs, Holding, Input
                    _ => 0,
                };

                if max_index == 0 {
                    return Ok(());
                }

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
                bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
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
                    if let crate::tui::status::Page::ModbusDashboard { cursor, .. } = &status.page {
                        Ok(*cursor)
                    } else {
                        Ok(types::cursor::ModbusDashboardCursor::AddLine)
                    }
                })?;

                let max_index = match current_cursor {
                    types::cursor::ModbusDashboardCursor::ModbusMode => 2, // Master, Slave
                    types::cursor::ModbusDashboardCursor::MasterSourceKind => {
                        types::modbus::ModbusMasterDataSourceKind::all().len()
                    }
                    types::cursor::ModbusDashboardCursor::MasterSourceValue => {
                        // Count available ports for TransparentForward selector
                        read_status(|status| {
                            if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
                                if let Some(port_name) = status.ports.order.get(*selected_port) {
                                    // Count ports excluding the current one
                                    let count = status.ports.order.iter()
                                        .filter(|p| *p != port_name)
                                        .count();
                                    return Ok(count);
                                }
                            }
                            Ok(0)
                        }).unwrap_or(0)
                    }
                    types::cursor::ModbusDashboardCursor::RegisterMode { .. } => 4, // Coils, DiscreteInputs, Holding, Input
                    _ => 0,
                };

                if max_index == 0 {
                    return Ok(());
                }

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
                bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
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
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    let port_name_opt = read_status(|status| Ok(status.ports.order.get(selected_port).cloned()))?;

    if let Some(port_name) = port_name_opt {
        if let Some(_port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
            match cursor {
                types::cursor::ModbusDashboardCursor::ModbusMode => {
                    // Apply global mode changes to all stations in this port
                    let new_mode = if selected_index == 0 {
                        ModbusConnectionMode::default_master()
                    } else {
                        ModbusConnectionMode::default_slave()
                    };

                    let mut should_restart = false;
                    let connection_mode = if new_mode.is_master() {
                        StationMode::Master
                    } else {
                        StationMode::Slave
                    };
                    write_status(|status| {
                        let port = status
                            .ports
                            .map
                            .get_mut(&port_name)
                            .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                        // evaluate occupancy before taking a mutable borrow of port.config
                        let was_occupied_by_this = matches!(port.state, PortState::OccupiedByThis);

                        let types::port::PortConfig::Modbus { mode, .. } = &mut port.config;
                        let old_was_master = mode.is_master();
                        let new_is_master = new_mode.is_master();

                        if old_was_master != new_is_master && was_occupied_by_this {
                            should_restart = true;
                        }

                        *mode = new_mode.clone();
                        log::info!("Updated global connection mode to {:?}", mode.is_master());

                        // Mark as modified - will trigger restart if needed
                        port.config_modified = true;
                        Ok(())
                    })?;

                    if should_restart {
                        let translations = lang();
                        let reason = format!(
                            "{} {}",
                            translations
                                .tabs
                                .log
                                .runtime_restart_reason_connection_mode_change
                                .clone(),
                            new_mode
                        );
                        crate::tui::append_runtime_restart_log(&port_name, reason, connection_mode);
                        return Ok(Some(port_name.clone()));
                    }
                }
                types::cursor::ModbusDashboardCursor::MasterSourceKind => {
                    let new_kind = ModbusMasterDataSourceKind::from_index(selected_index);

                    let mut should_restart = false;
                    write_status(|status| {
                        let port = status
                            .ports
                            .map
                            .get_mut(&port_name)
                            .ok_or_else(|| anyhow::anyhow!("Port not found"))?;

                        let types::port::PortConfig::Modbus { master_source, .. } =
                            &mut port.config;

                        let old_kind = master_source.kind();
                        if old_kind != new_kind {
                            if matches!(port.state, PortState::OccupiedByThis) {
                                should_restart = true;
                            }

                            master_source.set_kind(new_kind);
                            port.config_modified = true;
                            log::info!("Updated master data source kind to {:?}", new_kind);
                        }

                        Ok(())
                    })?;

                    if should_restart {
                        let translations = lang();
                        let reason = format!(
                            "{} {}",
                            translations
                                .tabs
                                .log
                                .runtime_restart_reason_data_source_change
                                .clone(),
                            new_kind
                        );
                        crate::tui::append_runtime_restart_log(
                            &port_name,
                            reason,
                            StationMode::Master,
                        );
                        return Ok(Some(port_name.clone()));
                    }
                }
                types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
                    // Apply register mode changes
                    let new_mode = RegisterMode::from_u8((selected_index as u8) + 1);

                    let mut should_restart = false;
                    let mut connection_mode = StationMode::Master;
                    write_status(|status| {
                        let port = status
                            .ports
                            .map
                            .get_mut(&port_name)
                            .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                        let types::port::PortConfig::Modbus {
                            mode,
                            master_source: _,
                            stations,
                        } = &mut port.config;
                        let mut all_items: Vec<_> = stations.iter_mut().collect();
                        if let Some(item) = all_items.get_mut(index) {
                            item.register_mode = new_mode;
                            port.config_modified = true; // Mark as modified
                            connection_mode = if mode.is_master() {
                                StationMode::Master
                            } else {
                                StationMode::Slave
                            };
                            // Schedule restart if port is running
                            if matches!(port.state, types::port::PortState::OccupiedByThis) {
                                should_restart = true;
                            }
                            log::info!("Updated register mode for index {index} to {new_mode:?}");
                        }
                        Ok(())
                    })?;

                    if should_restart {
                        let translations = lang();
                        let reason = format!(
                            "{} {}",
                            translations
                                .tabs
                                .log
                                .runtime_restart_reason_station_mode_change
                                .clone(),
                            new_mode
                        );
                        crate::tui::append_runtime_restart_log(&port_name, reason, connection_mode);
                        return Ok(Some(port_name.clone()));
                    }
                }
                types::cursor::ModbusDashboardCursor::MasterSourceValue => {
                    // Handle TransparentForward port selector
                    let mut should_restart = false;
                    write_status(|status| {
                        let port_data = status
                            .ports
                            .map
                            .get_mut(&port_name)
                            .ok_or_else(|| anyhow::anyhow!("Port not found"))?;

                        let types::port::PortConfig::Modbus { master_source, .. } =
                            &mut port_data.config;

                        if let ModbusMasterDataSource::TransparentForward { port: existing } = master_source {
                            // Get list of available ports (excluding current port)
                            let available_ports: Vec<String> = status.ports.order.iter()
                                .filter(|p| *p != &port_name)
                                .cloned()
                                .collect();
                            
                            if let Some(selected_port_name) = available_ports.get(selected_index) {
                                let new_value = Some(selected_port_name.clone());
                                if *existing != new_value {
                                    *existing = new_value;
                                    port_data.config_modified = true;
                                    if matches!(port_data.state, types::port::PortState::OccupiedByThis) {
                                        should_restart = true;
                                    }
                                }
                            }
                        }

                        Ok(())
                    })?;

                    if should_restart {
                        let translations = lang();
                        let reason = format!(
                            "{} {}",
                            translations
                                .tabs
                                .log
                                .runtime_restart_reason_data_source_change
                                .clone(),
                            ModbusMasterDataSourceKind::TransparentForward
                        );
                        crate::tui::append_runtime_restart_log(
                            &port_name,
                            reason,
                            StationMode::Master,
                        );
                        return Ok(Some(port_name.clone()));
                    }
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
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    let port_name_opt = read_status(|status| Ok(status.ports.order.get(selected_port).cloned()))?;

    if let Some(port_name) = port_name_opt {
        if let Some(_port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
            match cursor {
                types::cursor::ModbusDashboardCursor::MasterSourceValue => {
                    let trimmed = value.trim().to_string();
                    let mut should_restart = false;
                    let mut updated_kind: Option<ModbusMasterDataSourceKind> = None;

                    write_status(|status| {
                        let port_data = status
                            .ports
                            .map
                            .get_mut(&port_name)
                            .ok_or_else(|| anyhow::anyhow!("Port not found"))?;

                        let types::port::PortConfig::Modbus { master_source, .. } =
                            &mut port_data.config;
                        let current_kind = master_source.kind();
                        updated_kind = Some(current_kind);

                        match master_source {
                            ModbusMasterDataSource::TransparentForward { port: existing } => {
                                let new_value = if trimmed.is_empty() {
                                    None
                                } else {
                                    Some(trimmed.clone())
                                };
                                if *existing != new_value {
                                    *existing = new_value;
                                    port_data.config_modified = true;
                                    if matches!(
                                        port_data.state,
                                        types::port::PortState::OccupiedByThis
                                    ) {
                                        should_restart = true;
                                    }
                                }
                            }
                            ModbusMasterDataSource::MqttServer { url }
                            | ModbusMasterDataSource::HttpServer { url } => {
                                if *url != trimmed {
                                    *url = trimmed.clone();
                                    port_data.config_modified = true;
                                    if matches!(
                                        port_data.state,
                                        types::port::PortState::OccupiedByThis
                                    ) {
                                        should_restart = true;
                                    }
                                }
                            }
                            ModbusMasterDataSource::IpcPipe { path }
                            | ModbusMasterDataSource::PythonModule { path } => {
                                if *path != trimmed {
                                    *path = trimmed.clone();
                                    port_data.config_modified = true;
                                    if matches!(
                                        port_data.state,
                                        types::port::PortState::OccupiedByThis
                                    ) {
                                        should_restart = true;
                                    }
                                }
                            }
                            ModbusMasterDataSource::Manual => {}
                        }

                        Ok(())
                    })?;

                    if should_restart {
                        if let Some(kind) = updated_kind {
                            let translations = lang();
                            let reason = format!(
                                "{} {}",
                                translations
                                    .tabs
                                    .log
                                    .runtime_restart_reason_data_source_change
                                    .clone(),
                                kind
                            );
                            crate::tui::append_runtime_restart_log(
                                &port_name,
                                reason,
                                StationMode::Master,
                            );
                        }

                        bus.ui_tx
                            .send(UiToCore::RestartRuntime(port_name.clone()))
                            .map_err(|err| anyhow!(err))?;
                    }
                }
                types::cursor::ModbusDashboardCursor::StationId { index } => {
                    if let Ok(station_id) = value.parse::<u8>() {
                        write_status(|status| {
                            let port = status
                                .ports
                                .map
                                .get_mut(&port_name)
                                .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                            let types::port::PortConfig::Modbus {
                                mode: _,
                                master_source: _,
                                stations,
                            } = &mut port.config;
                            let mut all_items: Vec<_> = stations.iter_mut().collect();
                            if let Some(item) = all_items.get_mut(index) {
                                item.station_id = station_id;
                                port.config_modified = true; // Mark as modified
                                log::info!("Updated station ID for index {index} to {station_id}");
                            }
                            Ok(())
                        })?;
                    }
                }
                types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
                    if let Ok(start_address) = value.parse::<u16>() {
                        write_status(|status| {
                            let port = status
                                .ports
                                .map
                                .get_mut(&port_name)
                                .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                            let types::port::PortConfig::Modbus {
                                mode: _,
                                master_source: _,
                                stations,
                            } = &mut port.config;
                            let mut all_items: Vec<_> = stations.iter_mut().collect();
                            if let Some(item) = all_items.get_mut(index) {
                                item.register_address = start_address;
                                port.config_modified = true; // Mark as modified
                                log::info!("Updated register start address for index {index} to {start_address}");
                            }
                            Ok(())
                        })?;
                    }
                }
                types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
                    if let Ok(length) = value.parse::<u16>() {
                        write_status(|status| {
                            let port = status
                                .ports
                                .map
                                .get_mut(&port_name)
                                .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                            let types::port::PortConfig::Modbus {
                                mode: _,
                                master_source: _,
                                stations,
                            } = &mut port.config;
                            let mut all_items: Vec<_> = stations.iter_mut().collect();
                            if let Some(item) = all_items.get_mut(index) {
                                item.register_length = length;
                                item.last_values.resize(length as usize, 0);
                                port.config_modified = true; // Mark as modified
                                log::info!("Updated register length for index {index} to {length}");
                            }
                            Ok(())
                        })?;
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

                    if let Ok(mut register_value) = parsed_value {
                        let mut subprocess_info_snapshot: Option<PortSubprocessInfo> = None;
                        let mut payload: Option<(String, u8, u16, Vec<u16>)> = None;

                        write_status(|status| {
                            let port = status
                                .ports
                                .map
                                .get_mut(&port_name)
                                .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                            let owner_info = port.subprocess_info.clone();

                            let types::port::PortConfig::Modbus {
                                mode,
                                master_source: _,
                                stations,
                            } = &mut port.config;
                            if let Some(item) = stations.get_mut(slave_index) {
                                if item.register_length as usize != item.last_values.len() {
                                    item.last_values.resize(item.register_length as usize, 0);
                                }

                                let idx = register_index;
                                if idx >= item.last_values.len() {
                                    item.last_values.resize(idx + 1, 0);
                                }

                                let (sanitized_value, register_type) = match item.register_mode {
                                    RegisterMode::Holding => (register_value, "holding"),
                                    RegisterMode::Input => (register_value, "input"),
                                    RegisterMode::Coils => {
                                        (if register_value == 0 { 0 } else { 1 }, "coil")
                                    }
                                    RegisterMode::DiscreteInputs => {
                                        (if register_value == 0 { 0 } else { 1 }, "discrete")
                                    }
                                };

                                register_value = sanitized_value;
                                item.last_values[idx] = sanitized_value;

                                let register_addr = item.register_address + register_index as u16;
                                payload = Some((
                                    register_type.to_string(),
                                    item.station_id,
                                    register_addr,
                                    vec![sanitized_value],
                                ));

                                if matches!(mode, ModbusConnectionMode::Slave { .. }) {
                                    let needs_enqueue = !matches!(
                                        owner_info.as_ref(),
                                        Some(info)
                                            if info.mode == PortSubprocessMode::MasterProvide
                                    );

                                    if needs_enqueue {
                                        enqueue_slave_write(item, register_addr, sanitized_value);
                                    }
                                }
                            }

                            subprocess_info_snapshot = owner_info;
                            Ok(())
                        })?;

                        if let (
                            Some(cli_info),
                            Some((register_type, station_id, start_address, values)),
                        ) = (&subprocess_info_snapshot, &payload)
                        {
                            log::info!(
                                "📤 Sending RegisterUpdate to core: port={port_name}, station={station_id}, type={register_type}, addr={start_address}, values={values:?}"
                            );
                            if let Err(err) = bus.ui_tx.send(UiToCore::SendRegisterUpdate {
                                port_name: port_name.clone(),
                                station_id: *station_id,
                                register_type: register_type.clone(),
                                start_address: *start_address,
                                values: values.clone(),
                            }) {
                                log::error!("❌ Failed to send RegisterUpdate to channel: {err}");
                            }

                            // WORKAROUND: Also directly update the data source file if in MasterProvide mode
                            // This bypasses potential IPC issues
                            if let Some(data_source_path) = &cli_info.data_source_path {
                                if let Err(err) = update_cli_data_source_file(
                                    data_source_path,
                                    *start_address,
                                    values,
                                ) {
                                    log::warn!(
                                            "Failed to directly update data source file {data_source_path}: {err}"
                                        );
                                } else {
                                    log::info!(
                                        "✅ Directly updated data source file: {data_source_path}"
                                    );
                                }
                            }
                        } else {
                            log::debug!(
                                "🚫 Not sending RegisterUpdate: subprocess_info_snapshot={:?}, payload={:?}",
                                subprocess_info_snapshot
                                    .as_ref()
                                    .map(|info| format!("CliSubprocess(mode={:?})", info.mode)),
                                payload.is_some()
                            );
                        }
                    }
                }
                types::cursor::ModbusDashboardCursor::RequestInterval => {
                    if let Ok(interval_ms) = value.parse::<u32>() {
                        write_status(|status| {
                            let port = status
                                .ports
                                .map
                                .get_mut(&port_name)
                                .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                            port.serial_config.request_interval_ms = interval_ms;
                            port.config_modified = true; // Mark as modified
                            log::info!("Updated request interval to {interval_ms} ms");
                            Ok(())
                        })?;
                    }
                }
                types::cursor::ModbusDashboardCursor::Timeout => {
                    if let Ok(timeout_ms) = value.parse::<u32>() {
                        write_status(|status| {
                            let port = status
                                .ports
                                .map
                                .get_mut(&port_name)
                                .ok_or_else(|| anyhow::anyhow!("Port not found"))?;
                            port.serial_config.timeout_ms = timeout_ms;
                            port.config_modified = true; // Mark as modified
                            log::info!("Updated timeout to {timeout_ms} ms");
                            Ok(())
                        })?;
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

/// Directly update the CLI data source file with new register values
/// This is a workaround for IPC timing issues in test environments
fn update_cli_data_source_file(path: &str, start_address: u16, values: &[u16]) -> Result<()> {
    use std::{fs, path::PathBuf};

    let path_buf = PathBuf::from(path);

    // Read the current data
    let content = fs::read_to_string(&path_buf)?;
    let mut data: serde_json::Value = serde_json::from_str(&content)?;

    // Update the values array, expanding if necessary
    if let Some(values_array) = data.get_mut("values").and_then(|v| v.as_array_mut()) {
        let start_idx = start_address as usize;

        // Ensure the array is large enough
        let required_len = start_idx + values.len();
        while values_array.len() < required_len {
            values_array.push(serde_json::json!(0));
        }

        // Update the values
        for (i, &value) in values.iter().enumerate() {
            let idx = start_idx + i;
            values_array[idx] = serde_json::json!(value);
        }

        // Write back to file
        let updated = serde_json::to_string(&data)?;
        fs::write(&path_buf, updated)?;
    }

    Ok(())
}
