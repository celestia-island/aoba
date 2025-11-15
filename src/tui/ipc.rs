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
            log::debug!("CLI[{port_name}]: ModbusData {direction} {data}");

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
        IpcMessage::StationsUpdate { stations_data, .. } => {
            log::info!(
                "CLI[{port_name}]: StationsUpdate received, {} bytes",
                stations_data.len()
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
                                        let station_index = modbus_stations.iter().position(|s| {
                                            s.station_id == station_config.station_id
                                                && s.register_mode == register_mode
                                                && s.register_address == range.address_start
                                                && s.register_length == range.length
                                        });

                                        if let Some(idx) = station_index {
                                            modbus_stations[idx].last_values = range.initial_values.clone();
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
    }
    Ok(())
}
