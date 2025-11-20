use chrono::Local;
use std::process::ExitStatus;

use crate::{
    cli::status::CliMode,
    tui::status::{
        modbus::{RegisterMode, StationMode},
        port::{
            PortCommunicationDirection, PortConfig, PortLifecycleLog, PortLifecyclePhase,
            PortLogEntry, PortLogMetadata, PortManagementEvent, PortManagementLog,
        },
    },
    utils::i18n::lang,
};

#[derive(Default)]
pub(crate) struct ModbusLogHints {
    pub(crate) station_id: Option<u8>,
    pub(crate) register_mode: Option<RegisterMode>,
    pub(crate) register_start: Option<u16>,
    pub(crate) register_quantity: Option<u16>,
    pub(crate) config_index: Option<u16>,
    pub(crate) success: Option<bool>,
    pub(crate) error: Option<String>,
}

pub(crate) fn append_port_log_internal(
    port_name: &str,
    raw: String,
    metadata: Option<PortLogMetadata>,
) {
    let entry = PortLogEntry {
        when: Local::now(),
        raw,
        parsed: None,
        metadata,
    };

    if let Err(err) = crate::tui::status::write_status(|status| {
        if let Some(port) = status.ports.map.get_mut(port_name) {
            port.logs.push(entry.clone());
            if port.logs.len() > 1000 {
                let excess = port.logs.len() - 1000;
                port.logs.drain(0..excess);
            }
        }
        Ok(())
    }) {
        log::warn!("append_port_log: failed to persist log entry for {port_name}: {err}");
    }
}

pub(crate) fn append_lifecycle_log(
    port_name: &str,
    phase: PortLifecyclePhase,
    note: Option<String>,
) {
    let summary = match phase {
        PortLifecyclePhase::Created => lang().tabs.log.lifecycle_started.clone(),
        PortLifecyclePhase::Shutdown => lang().tabs.log.lifecycle_shutdown.clone(),
        PortLifecyclePhase::Restarted => lang().tabs.log.lifecycle_restarted.clone(),
        PortLifecyclePhase::Failed => lang().tabs.log.lifecycle_failed.clone(),
    };

    let note = note.filter(|value| !value.trim().is_empty());
    let metadata = PortLogMetadata::Lifecycle(PortLifecycleLog { phase, note });
    append_port_log_internal(port_name, summary, Some(metadata));
}

pub(crate) fn append_modbus_log(
    port_name: &str,
    direction: &str,
    data: &str,
    hints: Option<ModbusLogHints>,
) {
    use crate::tui::status::modbus::StationMode;
    use crate::tui::status::port::PortCommunicationLog;

    let payload = crate::core::helpers::parse_hex_payload(data);
    let direction_enum = match direction {
        "tx" => PortCommunicationDirection::Outbound,
        _ => PortCommunicationDirection::Inbound,
    };

    let translations = lang();
    let summary = match direction_enum {
        PortCommunicationDirection::Outbound => translations.tabs.log.comm_send.clone(),
        PortCommunicationDirection::Inbound => translations.tabs.log.comm_receive.clone(),
    };

    let ModbusLogHints {
        station_id: hint_station_id,
        register_mode: hint_register_mode,
        register_start: hint_register_start,
        register_quantity: hint_register_quantity,
        config_index: hint_config_index,
        success: hint_success,
        error: hint_failure_reason,
    } = hints.unwrap_or_default();

    let mut station_id = hint_station_id;
    let mut register_mode = hint_register_mode;
    let mut register_start = hint_register_start;
    let mut register_quantity = hint_register_quantity;
    let mut config_index: Option<u16> = hint_config_index;
    let mut parse_error: Option<String> = None;
    let success_hint = hint_success;
    let mut failure_reason = hint_failure_reason;

    if failure_reason
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        failure_reason = None;
    }

    if payload.len() >= 2 {
        if station_id.is_none() {
            station_id = Some(payload[0]);
        }
        let function_code = payload[1];
        if register_mode.is_none() {
            register_mode = crate::tui::map_function_to_register_mode(function_code);
        }

        match function_code {
            0x01..=0x04 => {
                if payload.len() >= 6 {
                    if register_start.is_none() {
                        register_start = Some(u16::from_be_bytes([payload[2], payload[3]]));
                    }
                    if register_quantity.is_none() {
                        register_quantity = Some(u16::from_be_bytes([payload[4], payload[5]]));
                    }
                } else if payload.len() >= 3 {
                    let byte_count = payload[2] as u16;
                    if register_quantity.is_none() {
                        register_quantity = match function_code {
                            0x01 | 0x02 => Some(byte_count * 8),
                            0x03 | 0x04 => Some(byte_count / 2),
                            _ => None,
                        };
                    }
                } else {
                    parse_error = Some(translations.tabs.log.comm_error_frame_short.clone());
                }
            }
            0x05 | 0x06 => {
                if payload.len() >= 6 {
                    if register_start.is_none() {
                        register_start = Some(u16::from_be_bytes([payload[2], payload[3]]));
                    }
                    if register_quantity.is_none() {
                        register_quantity = Some(1);
                    }
                } else {
                    parse_error = Some(translations.tabs.log.comm_error_frame_short.clone());
                }
            }
            0x0F | 0x10 => {
                if payload.len() >= 6 {
                    if register_start.is_none() {
                        register_start = Some(u16::from_be_bytes([payload[2], payload[3]]));
                    }
                    if register_quantity.is_none() {
                        register_quantity = Some(u16::from_be_bytes([payload[4], payload[5]]));
                    }
                } else {
                    parse_error = Some(translations.tabs.log.comm_error_frame_short.clone());
                }
            }
            _ => {
                if parse_error.is_none() {
                    parse_error = Some(format!(
                        "{} 0x{function_code:02X}",
                        translations.tabs.log.comm_error_unsupported_func.clone()
                    ));
                }
            }
        }
    } else {
        parse_error = Some(translations.tabs.log.comm_error_frame_short.clone());
    }

    let mut role = StationMode::Master;
    let mut role_confident = false;
    let status_lookup = crate::tui::status::read_status(|status| {
        if let Some(port) = status.ports.map.get(port_name) {
            match &port.config {
                PortConfig::Modbus { mode, stations, .. } => {
                    let role = if mode.is_master() {
                        StationMode::Master
                    } else {
                        StationMode::Slave
                    };

                    let matched = if let (Some(sid), Some(reg_mode)) = (station_id, register_mode) {
                        let mut candidate: Option<(usize, u16, u16)> = None;

                        for (idx, item) in stations.iter().enumerate() {
                            if item.station_id != sid || item.register_mode != reg_mode {
                                continue;
                            }

                            if let Some(start) = register_start {
                                if item.register_address == start {
                                    candidate =
                                        Some((idx, item.register_address, item.register_length));
                                    break;
                                }
                            }

                            if candidate.is_none() {
                                candidate =
                                    Some((idx, item.register_address, item.register_length));
                            }
                        }

                        candidate.map(|(idx, start, length)| (idx as u16 + 1, start, length))
                    } else {
                        None
                    };

                    Ok(Some((role, matched)))
                }
            }
        } else {
            Ok(None)
        }
    });

    match status_lookup {
        Ok(Some((detected_role, matched))) => {
            role = detected_role;
            role_confident = true;
            if let Some((index, start, length)) = matched {
                if config_index.is_none() {
                    config_index = Some(index);
                }

                if register_start.is_none() {
                    register_start = Some(start);
                }
                if register_quantity.is_none() {
                    register_quantity = Some(length);
                }
            }
        }
        Ok(None) => {}
        Err(err) => {
            log::warn!("append_modbus_log: failed to read status for {port_name}: {err}");
        }
    }

    if role_confident {
        let skip = matches!(
            (role, direction_enum),
            (StationMode::Slave, PortCommunicationDirection::Inbound)
        );

        if skip {
            return;
        }
    }

    let register_end = register_start
        .and_then(|start| register_quantity.map(|len| start.saturating_add(len.saturating_sub(1))));

    let full_payload_hex = if payload.is_empty() {
        String::new()
    } else {
        payload
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    if failure_reason.is_none() {
        failure_reason = parse_error.clone();
    }

    if success_hint == Some(true) {
        parse_error = None;
        failure_reason = None;
    }

    if success_hint == Some(false) && failure_reason.is_none() {
        failure_reason = Some(translations.tabs.log.reason_none.clone());
    }

    let metadata = PortCommunicationLog {
        direction: direction_enum,
        role,
        station_id,
        register_mode,
        config_index,
        register_start,
        register_end,
        register_quantity,
        payload,
        parse_error,
        success_hint,
        failure_reason,
    };

    if let Some(err) = &metadata.parse_error {
        log::warn!(
            "CLI[{port_name}]: unable to parse modbus frame ({direction}): {err}; frame={full_payload_hex}"
        );
    } else if matches!(metadata.success_hint, Some(false)) {
        if let Some(reason) = &metadata.failure_reason {
            log::warn!(
                "CLI[{port_name}]: reported failed modbus operation ({direction}): {reason}; frame={full_payload_hex}"
            );
        }
    }

    append_port_log_internal(
        port_name,
        summary,
        Some(PortLogMetadata::Communication(metadata)),
    );
}

pub(crate) fn append_management_log(port_name: &str, summary: String, event: PortManagementEvent) {
    let metadata = PortLogMetadata::Management(PortManagementLog { event });
    append_port_log_internal(port_name, summary, Some(metadata));
}

pub(crate) fn append_stations_update_logs(
    port_name: &str,
    station_configs: &[crate::cli::config::StationConfig],
) {
    let translations = lang();
    let mut next_index: u16 = 1;

    for station in station_configs {
        let mode = station.mode;
        let station_id = station.station_id;

        let mut append_from_ranges =
            |register_mode: RegisterMode,
             ranges: &[crate::protocol::status::types::modbus::RegisterRange]| {
                for range in ranges {
                    let start = range.address_start;
                    let end = if range.length == 0 {
                        start
                    } else {
                        start.saturating_add(range.length.saturating_sub(1))
                    };

                    let summary = format!(
                        "{} #{:04X}",
                        translations.tabs.log.stations_update_summary, next_index
                    );

                    append_management_log(
                        port_name,
                        summary,
                        PortManagementEvent::ConfigSync {
                            mode,
                            config_index: next_index,
                            station_id,
                            register_mode,
                            address_start: start,
                            address_end: end,
                            success: true,
                            error: None,
                        },
                    );

                    next_index = next_index.saturating_add(1);
                }
            };

        append_from_ranges(RegisterMode::Coils, &station.map.coils);
        append_from_ranges(RegisterMode::DiscreteInputs, &station.map.discrete_inputs);
        append_from_ranges(RegisterMode::Holding, &station.map.holding);
        append_from_ranges(RegisterMode::Input, &station.map.input);
    }

    if next_index == 1 {
        let summary = translations.tabs.log.stations_update_summary.clone();
        append_management_log(
            port_name,
            summary,
            PortManagementEvent::ConfigSync {
                mode: StationMode::Master,
                config_index: 0,
                station_id: 0,
                register_mode: RegisterMode::Holding,
                address_start: 0,
                address_end: 0,
                success: true,
                error: None,
            },
        );
    }
}

pub(crate) fn append_stations_update_failed_log(port_name: &str, error: String) {
    let summary = lang().tabs.log.stations_update_failed_summary.clone();
    append_management_log(
        port_name,
        summary,
        PortManagementEvent::ConfigSync {
            mode: StationMode::Master,
            config_index: 0,
            station_id: 0,
            register_mode: RegisterMode::Holding,
            address_start: 0,
            address_end: 0,
            success: false,
            error: Some(error),
        },
    );
}

pub(crate) fn append_state_lock_request_log(port_name: &str, requester: &str) {
    append_management_log(
        port_name,
        lang().tabs.log.state_lock_request_summary.clone(),
        PortManagementEvent::StateLockRequest {
            requester: requester.to_string(),
        },
    );
}

pub(crate) fn append_state_lock_ack_log(port_name: &str, locked: bool) {
    append_management_log(
        port_name,
        lang().tabs.log.state_lock_ack_summary.clone(),
        PortManagementEvent::StateLockAck { locked },
    );
}

pub(crate) fn append_status_log(port_name: &str, status: &str, details: Option<&str>) {
    let summary = format!("{}: {status}", lang().tabs.log.status_summary);
    append_management_log(
        port_name,
        summary,
        PortManagementEvent::Status {
            status: status.to_string(),
            details: details.map(|d| d.to_string()),
        },
    );
}

pub(crate) fn append_cli_log_message(port_name: &str, level: &str, message: &str) {
    let summary = format!(
        "{} [{}]",
        lang().tabs.log.cli_log_summary,
        level.to_uppercase()
    );
    append_management_log(
        port_name,
        summary,
        PortManagementEvent::LogMessage {
            level: level.to_string(),
            message: message.to_string(),
        },
    );
}

pub(crate) fn append_subprocess_spawned_log(port_name: &str, mode: &CliMode, pid: Option<u32>) {
    let summary = lang().tabs.log.subprocess_spawned_summary.clone();
    append_management_log(
        port_name,
        summary,
        PortManagementEvent::SubprocessSpawned {
            mode: cli_mode_label(mode),
            pid,
        },
    );
}

pub(crate) fn append_subprocess_stopped_log(port_name: &str, reason: Option<String>) {
    append_management_log(
        port_name,
        lang().tabs.log.subprocess_stopped_summary.clone(),
        PortManagementEvent::SubprocessStopped { reason },
    );
}

pub(crate) fn append_subprocess_exited_log(port_name: &str, exit_status: Option<ExitStatus>) {
    let translations = lang();
    let summary = translations.tabs.log.subprocess_exited_summary.clone();

    let (success, mut detail) = match exit_status {
        Some(status) => {
            let success = status.success();
            let detail = if success {
                translations.tabs.log.subprocess_exit_success.clone()
            } else if let Some(code) = status.code() {
                let label = translations.tabs.log.subprocess_exit_code_label.clone();
                format!("{label} 0x{code:02X} ({code})")
            } else {
                translations.tabs.log.subprocess_exit_signal.clone()
            };
            (Some(success), detail)
        }
        None => (None, translations.tabs.log.reason_none.clone()),
    };

    // If process exited abnormally, append recent stderr logs to the detail
    if success != Some(true) {
        if let Ok(stderr_summary) = crate::tui::status::read_status(|status| {
            if let Some(port) = status.ports.map.get(port_name) {
                if !port.cli_stderr_logs.is_empty() {
                    // Get the last 3 stderr lines
                    let recent_logs: Vec<String> = port
                        .cli_stderr_logs
                        .iter()
                        .rev()
                        .take(3)
                        .map(|log| log.line.clone())
                        .collect();
                    if !recent_logs.is_empty() {
                        return Ok(Some(format!(
                            "\n[Recent stderr] {}",
                            recent_logs.join(" | ")
                        )));
                    }
                }
            }
            Ok(None)
        }) {
            if let Some(stderr_text) = stderr_summary {
                detail.push_str(&stderr_text);
            }
        }
    }

    append_management_log(
        port_name,
        summary,
        PortManagementEvent::SubprocessExited { success, detail },
    );
}

pub(crate) fn append_runtime_restart_log(
    port_name: &str,
    reason: String,
    connection_mode: StationMode,
) {
    let summary = lang().tabs.log.runtime_restart_summary.clone();
    append_management_log(
        port_name,
        summary,
        PortManagementEvent::RuntimeRestart {
            reason,
            connection_mode,
        },
    );
}

fn cli_mode_label(mode: &CliMode) -> String {
    match mode {
        CliMode::SlavePoll => lang().tabs.log.cli_mode_slave_poll.clone(),
        CliMode::SlaveListen => lang().tabs.log.cli_mode_slave_listen.clone(),
        CliMode::MasterProvide => lang().tabs.log.cli_mode_master_provide.clone(),
    }
}

pub(crate) fn map_function_to_register_mode(function_code: u8) -> Option<RegisterMode> {
    match function_code {
        0x01 | 0x05 | 0x0F => Some(RegisterMode::Coils),
        0x02 => Some(RegisterMode::DiscreteInputs),
        0x03 | 0x06 | 0x10 => Some(RegisterMode::Holding),
        0x04 => Some(RegisterMode::Input),
        _ => None,
    }
}
