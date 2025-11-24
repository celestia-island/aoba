use anyhow::Result;

use crate::{
    cli::config::StationConfig,
    protocol::ipc::IpcMessage,
    tui::{
        logs::{
            append_cli_log_message, append_lifecycle_log, append_modbus_log,
            append_state_lock_ack_log, append_state_lock_request_log,
            append_stations_update_failed_log, append_stations_update_logs, append_status_log,
        },
        status::{self as types},
    },
};

pub(crate) fn map_register_mode_hint(
    value: Option<String>,
) -> Option<crate::tui::status::modbus::RegisterMode> {
    value.and_then(|mode| match mode.as_str() {
        "Coils" => Some(crate::tui::status::modbus::RegisterMode::Coils),
        "DiscreteInputs" => Some(crate::tui::status::modbus::RegisterMode::DiscreteInputs),
        "Holding" => Some(crate::tui::status::modbus::RegisterMode::Holding),
        "Input" => Some(crate::tui::status::modbus::RegisterMode::Input),
        _ => None,
    })
}

pub(crate) fn handle_cli_ipc_message(port_name: &str, message: IpcMessage) -> Result<()> {
    match message {
        IpcMessage::PortOpened { .. } => {
            log::info!("CLI[{port_name}]: PortOpened received");
            append_lifecycle_log(
                port_name,
                crate::tui::status::port::PortLifecyclePhase::Created,
                None,
            );
        }
        IpcMessage::PortError { error, .. } => {
            let msg = format!("CLI subprocess error: {error}");
            log::warn!("CLI[{port_name}]: {msg}");
            append_lifecycle_log(
                port_name,
                crate::tui::status::port::PortLifecyclePhase::Failed,
                Some(error.clone()),
            );
            crate::tui::status::write_status(|status| {
                // Update port status to indicate startup failure
                if let Some(port) = status.ports.map.get_mut(port_name) {
                    // Clear subprocess info since it failed to start properly
                    port.subprocess_info = None;
                    port.state = types::port::PortState::Free;
                    // Set status indicator to NotStarted (error shown in bottom bar)
                    port.status_indicator = types::port::PortStatusIndicator::NotStarted;
                    log::info!(
                        "Port {port_name} status updated to NotStarted due to error: {error}"
                    );
                }
                // Also set global error message for user notification
                status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                    message: msg.clone(),
                    timestamp: chrono::Local::now(),
                });
                Ok(())
            })?;
        }
        IpcMessage::Shutdown { .. } => {
            log::info!("CLI[{port_name}]: Shutdown received");
            append_lifecycle_log(
                port_name,
                crate::tui::status::port::PortLifecyclePhase::Shutdown,
                None,
            );
        }
        IpcMessage::ModbusData {
            direction,
            data,
            station_id,
            register_mode,
            start_address,
            quantity,
            success,
            error,
            config_index,
            ..
        } => {
            let register_mode = map_register_mode_hint(register_mode);
            let sanitized_error = error.and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });

            let hints = crate::tui::logs::ModbusLogHints {
                station_id,
                register_mode,
                register_start: start_address,
                register_quantity: quantity,
                config_index,
                success,
                error: sanitized_error,
            };

            append_modbus_log(port_name, &direction, &data, Some(hints));
        }
        IpcMessage::Heartbeat { .. } => {}
        IpcMessage::StationsUpdate {
            stations_data,
            update_reason,
            ..
        } => {
            log::info!(
                "🔄 CLI[{port_name}]: StationsUpdate received, {} bytes, reason={:?}",
                stations_data.len(),
                update_reason
            );
            match postcard::from_bytes::<Vec<StationConfig>>(&stations_data) {
                Ok(stations) => {
                    log::info!("CLI[{port_name}]: Decoded {} stations", stations.len());
                    append_stations_update_logs(port_name, &stations);
                    crate::tui::status::write_status(|status| {
                        if let Some(port) = status.ports.map.get_mut(port_name) {
                            let types::port::PortConfig::Modbus {
                                mode: _,
                                master_source: _,
                                stations: ref mut modbus_stations,
                            } = &mut port.config;
                            for station_config in &stations {
                                let mut update_registers = |ranges: &[types::modbus::RegisterRange], register_mode: types::modbus::RegisterMode| {
                                    for range in ranges {
                                        // Find station by id, mode, and overlapping address range
                                        // Check if incoming range overlaps with station's configured range
                                        let station_index = modbus_stations.iter().position(|s| {
                                            if s.station_id != station_config.station_id || s.register_mode != register_mode {
                                                return false;
                                            }
                                            // Check if ranges overlap:
                                            // Station: [s.register_address, s.register_address + s.register_length)
                                            // Incoming: [range.address_start, range.address_start + range.length)
                                            let station_end = s.register_address + s.register_length;
                                            let range_end = range.address_start + range.length;
                                            // Ranges overlap if: start1 < end2 && start2 < end1
                                            s.register_address < range_end && range.address_start < station_end
                                        });

                                        if let Some(idx) = station_index {
                                            // Update last_values, but preserve pending writes
                                            // If a register has pending write, don't overwrite its value
                                            let pending_indices: std::collections::HashSet<usize> =
                                                modbus_stations[idx]
                                                    .pending_writes
                                                    .keys()
                                                    .copied()
                                                    .collect();
                                            log::info!(
                                                "📊 StationsUpdate: station_id={}, addr=0x{:04X}, {} registers, {} pending",
                                                modbus_stations[idx].station_id,
                                                modbus_stations[idx].register_address,
                                                range.initial_values.len(),
                                                pending_indices.len()
                                            );
                                            // Calculate offset: incoming range may start at different address than station
                                            let offset = (range.address_start - modbus_stations[idx].register_address) as usize;
                                            for (incoming_idx, &value) in range.initial_values.iter().enumerate() {
                                                // Convert to station's last_values index
                                                let station_idx = offset + incoming_idx;
                                                let old_value = modbus_stations[idx].last_values.get(station_idx).copied();
                                                // Only update if this register doesn't have a pending write
                                                if !pending_indices.contains(&station_idx) {
                                                    // Ensure last_values is large enough
                                                    if station_idx >= modbus_stations[idx].last_values.len() {
                                                        modbus_stations[idx].last_values.resize(station_idx + 1, 0);
                                                    }
                                                    modbus_stations[idx].last_values[station_idx] = value;
                                                    if let Some(old) = old_value {
                                                        if old != value {
                                                            log::info!(
                                                                "  📝 Register addr=0x{:04X} (idx={}): 0x{old:04X} → 0x{value:04X}",
                                                                range.address_start + incoming_idx as u16,
                                                                station_idx
                                                            );
                                                        }
                                                    } else {
                                                        log::info!(
                                                            "  📝 Register addr=0x{:04X} (idx={}): <new> → 0x{value:04X}",
                                                            range.address_start + incoming_idx as u16,
                                                            station_idx
                                                        );
                                                    }
                                                } else {
                                                    let pending_val = modbus_stations[idx].pending_writes.get(&station_idx).copied().unwrap_or(0);
                                                    log::info!(
                                                        "  ⏸️  Register addr=0x{:04X} (idx={}): Skipped (pending=0x{pending_val:04X}, incoming=0x{value:04X})",
                                                        range.address_start + incoming_idx as u16,
                                                        station_idx
                                                    );
                                                }
                                            }
                                        } else if !range.initial_values.is_empty() {
                                            let new_item = types::modbus::ModbusRegisterItem {
                                                station_id: station_config.station_id,
                                                register_mode,
                                                register_address: range.address_start,
                                                register_length: range.length,
                                                last_values: range.initial_values.clone(),
                                                req_success: 0,
                                                req_total: 0,
                                                next_poll_at: std::time::Instant::now(),
                                                last_request_time: None,
                                                last_response_time: None,
                                                pending_requests: Vec::new(),
                                                pending_writes: std::collections::HashMap::new(),
                                            };
                                            modbus_stations.push(new_item);
                                        }
                                    }
                                };

                                update_registers(
                                    &station_config.map.coils,
                                    types::modbus::RegisterMode::Coils,
                                );
                                update_registers(
                                    &station_config.map.discrete_inputs,
                                    types::modbus::RegisterMode::DiscreteInputs,
                                );
                                update_registers(
                                    &station_config.map.holding,
                                    types::modbus::RegisterMode::Holding,
                                );
                                update_registers(
                                    &station_config.map.input,
                                    types::modbus::RegisterMode::Input,
                                );
                            }
                        }
                        Ok(())
                    })?;
                }
                Err(err) => {
                    log::warn!("CLI[{port_name}]: Failed to deserialize stations data: {err}");
                    append_stations_update_failed_log(port_name, err.to_string());
                }
            }
        }
        IpcMessage::StateLockRequest { requester, .. } => {
            log::info!("CLI[{port_name}]: StateLockRequest from {requester}");
            append_state_lock_request_log(port_name, &requester);
        }
        IpcMessage::StateLockAck { locked, .. } => {
            log::info!("CLI[{port_name}]: StateLockAck locked={locked}");
            append_state_lock_ack_log(port_name, locked);
        }
        IpcMessage::Status {
            status, details, ..
        } => {
            let details_ref = details.as_deref();
            append_status_log(port_name, &status, details_ref);
            if let Some(details) = details {
                log::info!("CLI[{port_name}]: CLI status: {status} ({details})");
            } else {
                log::info!("CLI[{port_name}]: CLI status: {status}");
            }
        }
        IpcMessage::Log { level, message, .. } => {
            log::info!("CLI[{port_name}]: log[{level}] {message}");
            append_cli_log_message(port_name, &level, &message);
        }
        IpcMessage::RegisterWriteComplete {
            station_id,
            register_address,
            register_value,
            register_type,
            success,
            error,
            ..
        } => {
            log::info!(
                "✅ CLI[{port_name}]: RegisterWriteComplete station={station_id} addr=0x{register_address:04X} value=0x{register_value:04X} type={register_type} success={success}"
            );

            crate::tui::status::write_status(|status| {
                if let Some(port) = status.ports.map.get_mut(port_name) {
                    let types::port::PortConfig::Modbus {
                        mode: _,
                        master_source: _,
                        stations,
                    } = &mut port.config;

                    // Find the station and register to update
                    for station in stations.iter_mut() {
                        if station.station_id == station_id {
                            let register_index =
                                (register_address - station.register_address) as usize;

                            // Remove from pending writes
                            station.pending_writes.remove(&register_index);

                            if success {
                                // Update local value on success
                                if register_index < station.last_values.len() {
                                    let old_value = station.last_values[register_index];
                                    station.last_values[register_index] = register_value;
                                    log::info!(
                                        "✅ Write success: Updated register #{register_index}: 0x{old_value:04X} → 0x{register_value:04X}"
                                    );
                                } else {
                                    log::warn!(
                                        "⚠️  Register index {register_index} out of bounds (len={})",
                                        station.last_values.len()
                                    );
                                }
                            } else if let Some(err_msg) = &error {
                                // Show error message in status bar
                                status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                                    message: format!(
                                        "Write failed for register 0x{register_address:04X}: {err_msg}"
                                    ),
                                    timestamp: chrono::Local::now(),
                                });
                                log::warn!("❌ Write failed: {err_msg}");
                            }
                            break;
                        }
                    }

                    // Check if there are any remaining pending writes
                    let has_pending_writes = stations.iter().any(|s| !s.pending_writes.is_empty());

                    // Update status indicator
                    if !has_pending_writes {
                        // No more pending writes, restore to Running status
                        port.status_indicator = types::port::PortStatusIndicator::Running;
                    }
                    // If there are still pending writes, keep Syncing status
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}
