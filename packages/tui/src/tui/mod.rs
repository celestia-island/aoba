pub mod input;
pub mod persistence;
pub mod status;
pub mod subprocess;
pub mod ui;
pub mod utils;

use aoba_cli::{config::StationConfig, status::CliMode};
use aoba_protocol::{
    ipc::IpcMessage,
    status::debug_dump::{enable_debug_dump, start_status_dump_thread},
};

// Re-export Page for convenience since it's used throughout TUI code
pub use status::Page;

use anyhow::{anyhow, Result};
use chrono::Local;
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitStatus,
    sync::Arc,
    thread,
    time::Duration,
};

use ratatui::{backend::CrosstermBackend, layout::*, prelude::*};

use crate::i18n::lang;
use crate::tui::status::modbus::{RegisterMode, StationMode};
use crate::tui::status::port::{
    PortCommunicationDirection, PortCommunicationLog, PortConfig, PortData, PortLifecycleLog,
    PortLifecyclePhase, PortLogEntry, PortLogMetadata, PortManagementEvent, PortManagementLog,
    PortState, PortStatusIndicator, PortSubprocessInfo, PortSubprocessMode,
};
use crate::tui::status::Status;
use crate::tui::status::{self as types, TuiStatus};
use crate::tui::{
    subprocess::{CliSubprocessConfig, SubprocessManager},
    ui::components::error_msg::ui_error_set,
    utils::bus::{Bus, CoreToUi, UiToCore},
};

/// Create a stable data source file path for a specific station on a port.
///
/// The path is deterministic based on port name, station ID, register mode, and address,
/// allowing multiple Masters on the same port to maintain separate data files without
/// conflicts. The format is: `aoba_cli_{port}_s{station_id}_t{type:02}_a{addr:04X}.jsonl`
///
/// Example: `/tmp/aoba_cli__tmp_vcom1_s1_t03_a0000.jsonl`
fn create_cli_data_source_path(
    port_name: &str,
    station_id: u8,
    register_mode: RegisterMode,
    start_address: u16,
) -> PathBuf {
    let sanitized: String = port_name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => c,
            _ => '_',
        })
        .collect();
    let fallback = if sanitized.is_empty() {
        "port".to_string()
    } else {
        sanitized
    };

    // Convert register mode to 2-digit type code (01-04)
    let type_code = match register_mode {
        RegisterMode::Coils => 1,
        RegisterMode::DiscreteInputs => 2,
        RegisterMode::Holding => 3,
        RegisterMode::Input => 4,
    };

    let mut path = std::env::temp_dir();
    path.push(format!(
        "aoba_cli_{fallback}_s{station_id}_t{type_code:02}_a{start_address:04X}.jsonl"
    ));
    path
}

fn append_port_log_internal(port_name: &str, raw: String, metadata: Option<PortLogMetadata>) {
    let entry = PortLogEntry {
        when: Local::now(),
        raw,
        parsed: None,
        metadata,
    };

    if let Err(err) = self::status::write_status(|status| {
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

fn append_lifecycle_log(port_name: &str, phase: PortLifecyclePhase, note: Option<String>) {
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

#[derive(Default)]
struct ModbusLogHints {
    station_id: Option<u8>,
    register_mode: Option<RegisterMode>,
    register_start: Option<u16>,
    register_quantity: Option<u16>,
    config_index: Option<u16>,
    success: Option<bool>,
    error: Option<String>,
}

fn append_modbus_log(port_name: &str, direction: &str, data: &str, hints: Option<ModbusLogHints>) {
    let payload = parse_hex_payload(data);
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
            register_mode = map_function_to_register_mode(function_code);
        }

        match function_code {
            0x01 | 0x02 | 0x03 | 0x04 => {
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
    let status_lookup = self::status::read_status(|status| {
        if let Some(port) = status.ports.map.get(port_name) {
            match &port.config {
                PortConfig::Modbus { mode, stations } => {
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

fn map_register_mode_hint(value: Option<String>) -> Option<RegisterMode> {
    value.and_then(|mode| match mode.as_str() {
        "Coils" => Some(RegisterMode::Coils),
        "DiscreteInputs" => Some(RegisterMode::DiscreteInputs),
        "Holding" => Some(RegisterMode::Holding),
        "Input" => Some(RegisterMode::Input),
        _ => None,
    })
}

fn append_management_log(port_name: &str, summary: String, event: PortManagementEvent) {
    let metadata = PortLogMetadata::Management(PortManagementLog { event });
    append_port_log_internal(port_name, summary, Some(metadata));
}

fn append_stations_update_logs(port_name: &str, station_configs: &[StationConfig]) {
    let translations = lang();
    let mut next_index: u16 = 1;

    for station in station_configs {
        let mode = station.mode;
        let station_id = station.station_id;

        let mut append_from_ranges =
            |register_mode: RegisterMode,
             ranges: &[aoba_protocol::status::types::modbus::RegisterRange]| {
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

fn append_stations_update_failed_log(port_name: &str, error: String) {
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

fn append_state_lock_request_log(port_name: &str, requester: &str) {
    append_management_log(
        port_name,
        lang().tabs.log.state_lock_request_summary.clone(),
        PortManagementEvent::StateLockRequest {
            requester: requester.to_string(),
        },
    );
}

fn append_state_lock_ack_log(port_name: &str, locked: bool) {
    append_management_log(
        port_name,
        lang().tabs.log.state_lock_ack_summary.clone(),
        PortManagementEvent::StateLockAck { locked },
    );
}

fn append_status_log(port_name: &str, status: &str, details: Option<&str>) {
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

fn append_cli_log_message(port_name: &str, level: &str, message: &str) {
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

fn append_subprocess_spawned_log(port_name: &str, mode: &CliMode, pid: Option<u32>) {
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

fn append_subprocess_stopped_log(port_name: &str, reason: Option<String>) {
    append_management_log(
        port_name,
        lang().tabs.log.subprocess_stopped_summary.clone(),
        PortManagementEvent::SubprocessStopped { reason },
    );
}

fn append_subprocess_exited_log(port_name: &str, exit_status: Option<ExitStatus>) {
    let translations = lang();
    let summary = translations.tabs.log.subprocess_exited_summary.clone();

    let (success, detail) = match exit_status {
        Some(status) => {
            let success = status.success();
            let detail = if success {
                translations.tabs.log.subprocess_exit_success.clone()
            } else if let Some(code) = status.code() {
                let label = translations.tabs.log.subprocess_exit_code_label.clone();
                format!("{} 0x{code:02X} ({code})", label)
            } else {
                translations.tabs.log.subprocess_exit_signal.clone()
            };
            (Some(success), detail)
        }
        None => (None, translations.tabs.log.reason_none.clone()),
    };

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

fn map_function_to_register_mode(function_code: u8) -> Option<RegisterMode> {
    match function_code {
        0x01 | 0x05 | 0x0F => Some(RegisterMode::Coils),
        0x02 => Some(RegisterMode::DiscreteInputs),
        0x03 | 0x06 | 0x10 => Some(RegisterMode::Holding),
        0x04 => Some(RegisterMode::Input),
        _ => None,
    }
}

fn parse_hex_payload(data: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut current = String::new();

    for ch in data.chars() {
        if ch.is_ascii_hexdigit() {
            current.push(ch);
            if current.len() == 2 {
                if let Ok(value) = u8::from_str_radix(&current, 16) {
                    bytes.push(value);
                }
                current.clear();
            }
        } else {
            current.clear();
        }
    }

    bytes
}

fn register_mode_to_cli_arg(mode: types::modbus::RegisterMode) -> &'static str {
    use types::modbus::RegisterMode;

    match mode {
        RegisterMode::Coils => "coils",
        RegisterMode::DiscreteInputs => "discrete",
        RegisterMode::Holding => "holding",
        RegisterMode::Input => "input",
    }
}

fn cli_mode_to_port_mode(mode: &CliMode) -> PortSubprocessMode {
    match mode {
        CliMode::SlaveListen => PortSubprocessMode::SlaveListen,
        CliMode::SlavePoll => PortSubprocessMode::SlavePoll,
        CliMode::MasterProvide => PortSubprocessMode::MasterProvide,
    }
}

fn station_values_for_cli(station: &types::modbus::ModbusRegisterItem) -> Vec<u16> {
    let target_len = station.register_length as usize;
    if target_len == 0 {
        return Vec::new();
    }

    let mut values = station.last_values.clone();
    values.resize(target_len, 0);
    values
}

/// Initialize CLI data source for Master mode by merging all stations' data.
///
/// For a Master port with multiple stations (address ranges), this function:
/// 1. Collects all stations with the same station_id and register_mode
/// 2. Determines the overall address range (min to max)
/// 3. Merges all stations' data into a continuous array
/// 4. Writes the merged data to a single data file
///
/// The CLI subprocess will then serve this entire address range.
fn initialize_cli_data_source(
    port_name: &str,
    stations: &[types::modbus::ModbusRegisterItem],
) -> Result<(PathBuf, u16, u16, u16)> {
    if stations.is_empty() {
        return Err(anyhow::anyhow!(
            "No stations provided for data source initialization"
        ));
    }

    // Use first station's metadata as reference
    let first = &stations[0];
    let station_id = first.station_id;
    let register_mode = first.register_mode;

    // Find min and max addresses across all stations
    let mut min_addr = u16::MAX;
    let mut max_addr = 0u16;

    for station in stations {
        if station.station_id != station_id {
            log::warn!(
                "initialize_cli_data_source: skipping station with different ID {} (expected {})",
                station.station_id,
                station_id
            );
            continue;
        }
        if station.register_mode != register_mode {
            log::warn!(
                "initialize_cli_data_source: skipping station with different register mode (expected {register_mode:?})"
            );
            continue;
        }

        let start = station.register_address;
        let end = start + station.register_length;

        if start < min_addr {
            min_addr = start;
        }
        if end > max_addr {
            max_addr = end;
        }
    }

    let total_length = max_addr - min_addr;
    log::info!(
        "initialize_cli_data_source: merging {} stations for {port_name}, station_id={}, type={:?}, address range: 0x{:04X}-0x{:04X} (length={})",
        stations.len(),
        station_id,
        register_mode,
        min_addr,
        max_addr,
        total_length
    );

    // Create merged data array
    let mut merged_data = vec![0u16; total_length as usize];

    // Fill in data from each station
    for station in stations {
        if station.station_id != station_id || station.register_mode != register_mode {
            continue;
        }

        let start_offset = (station.register_address - min_addr) as usize;
        let station_values = station_values_for_cli(station);

        log::debug!(
            "  Merging station at 0x{:04X}, length={}, into offset {}",
            station.register_address,
            station_values.len(),
            start_offset
        );

        for (i, &value) in station_values.iter().enumerate() {
            let target_idx = start_offset + i;
            if target_idx < merged_data.len() {
                merged_data[target_idx] = value;
            }
        }
    }

    // Create path using first station's info (but covering full range)
    let path = create_cli_data_source_path(port_name, station_id, register_mode, min_addr);

    if let Err(err) = write_cli_data_snapshot(&path, &merged_data, true) {
        log::error!(
            "initialize_cli_data_source: failed to write merged snapshot for {port_name}: {err}"
        );
        return Err(err);
    }

    log::info!(
        "initialize_cli_data_source: created merged data source at {} (station_id={}, addr=0x{:04X}, length={})",
        path.display(),
        station_id,
        min_addr,
        total_length
    );

    Ok((path, station_id as u16, min_addr, total_length))
}

fn write_cli_data_snapshot(path: &PathBuf, values: &[u16], truncate: bool) -> Result<()> {
    let payload = serde_json::json!({ "values": values });
    let serialized = serde_json::to_string(&payload)?;

    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }

    let mut file = options.open(path)?;
    writeln!(file, "{serialized}")?;
    Ok(())
}

fn handle_cli_ipc_message(port_name: &str, message: IpcMessage) -> Result<()> {
    match message {
        IpcMessage::PortOpened { .. } => {
            log::info!("CLI[{port_name}]: PortOpened received");
            append_lifecycle_log(port_name, PortLifecyclePhase::Created, None);
        }
        IpcMessage::PortError { error, .. } => {
            let msg = format!("CLI subprocess error: {error}");
            log::warn!("CLI[{port_name}]: {msg}");
            append_lifecycle_log(port_name, PortLifecyclePhase::Failed, Some(error.clone()));
            self::status::write_status(|status| {
                status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                    message: msg.clone(),
                    timestamp: chrono::Local::now(),
                });
                Ok(())
            })?;
        }
        IpcMessage::Shutdown { .. } => {
            log::info!("CLI[{port_name}]: Shutdown received");
            append_lifecycle_log(port_name, PortLifecyclePhase::Shutdown, None);
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

            let hints = ModbusLogHints {
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
        IpcMessage::Heartbeat { .. } => {
            // Heartbeat can be ignored for now or used for future monitoring
        }
        IpcMessage::StationsUpdate { stations_data, .. } => {
            log::info!(
                "CLI[{port_name}]: StationsUpdate received, {} bytes",
                stations_data.len()
            );

            // Deserialize and update the port's station configuration
            match postcard::from_bytes::<Vec<StationConfig>>(&stations_data) {
                Ok(stations) => {
                    log::info!("CLI[{port_name}]: Decoded {} stations", stations.len());
                    append_stations_update_logs(port_name, &stations);

                    // Apply the stations update to the port's ModbusRegisterItem list
                    self::status::write_status(|status| {
                        if let Some(port) = status.ports.map.get_mut(port_name) {
                            let types::port::PortConfig::Modbus {
                                mode: _,
                                stations: ref mut modbus_stations,
                            } = &mut port.config;
                            log::info!(
                                "CLI[{port_name}]: Applying {} station configs to port",
                                stations.len()
                            );

                            // Convert each StationConfig to ModbusRegisterItem
                            for station_config in &stations {
                                // Find or create the corresponding ModbusRegisterItem
                                // For simplicity, we'll update the first matching station by ID
                                // or create a new one if it doesn't exist

                                // Helper function to update register values from RegisterRange
                                let mut update_registers = |
                                    ranges: &[types::modbus::RegisterRange],
                                    register_mode: types::modbus::RegisterMode,
                                | {
                                    for range in ranges {
                                        let station_index = modbus_stations.iter().position(|s| {
                                            s.station_id == station_config.station_id
                                                && s.register_mode == register_mode
                                                && s.register_address == range.address_start
                                                && s.register_length == range.length
                                        });

                                        if let Some(idx) = station_index {
                                            // Update existing station
                                            modbus_stations[idx].last_values =
                                                range.initial_values.clone();
                                            log::debug!(
                                                "CLI[{port_name}]: Updated station {} {:?} at 0x{:04X} with {} values",
                                                station_config.station_id,
                                                register_mode,
                                                range.address_start,
                                                range.initial_values.len()
                                            );
                                        } else if !range.initial_values.is_empty() {
                                            // Create new station entry
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
                                            log::debug!(
                                                "CLI[{port_name}]: Created new station {} {:?} at 0x{:04X} with {} values",
                                                station_config.station_id,
                                                register_mode,
                                                range.address_start,
                                                range.initial_values.len()
                                            );
                                        }
                                    }
                                };

                                // Update all register types from the station config
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

                            log::info!("CLI[{port_name}]: Successfully applied stations update");
                        }
                        Ok(())
                    })?;
                }
                Err(err) => {
                    log::warn!("CLI[{port_name}]: Failed to deserialize stations data: {err}");

                    if log::log_enabled!(log::Level::Debug) {
                        let preview = stations_data
                            .iter()
                            .take(128)
                            .map(|byte| format!("{byte:02X}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        log::debug!(
                            "CLI[{port_name}]: Stations data hex (truncated to 128 bytes): {preview}"
                        );
                    }

                    append_stations_update_failed_log(port_name, err.to_string());
                }
            }
        }
        IpcMessage::StateLockRequest { requester, .. } => {
            log::info!("CLI[{port_name}]: StateLockRequest from {requester}");
            append_state_lock_request_log(port_name, &requester);

            // Basic state locking mechanism implementation
            // For now, we always acknowledge the lock request immediately
            // In a more complex implementation, this would check if the state
            // is currently being modified and potentially queue the request
            log::debug!("CLI[{port_name}]: Auto-acknowledging state lock request");

            // In a full implementation, we would:
            // 1. Check if state is currently locked by another process
            // 2. Queue the request if locked, or grant it immediately
            // 3. Send back a StateLockAck message via IPC
            // For now, we just log the request
        }
        IpcMessage::StateLockAck { locked, .. } => {
            log::info!("CLI[{port_name}]: StateLockAck locked={locked}");
            append_state_lock_ack_log(port_name, locked);

            // Handle state lock acknowledgment
            // This message is sent by CLI to acknowledge that it has locked or unlocked its state
            // The TUI can use this to coordinate updates safely
            if locked {
                log::debug!("CLI[{port_name}]: State is now locked for updates");
                // In a full implementation, we would mark the port as locked
                // and prevent local modifications until unlocked
            } else {
                log::debug!("CLI[{port_name}]: State is now unlocked");
                // In a full implementation, we would allow local modifications again
            }
        }
        IpcMessage::Status {
            status, details, ..
        } => {
            let details_ref = details.as_deref();
            append_status_log(port_name, &status, details_ref);

            let msg = if let Some(details) = details {
                format!("CLI status: {status} ({details})")
            } else {
                format!("CLI status: {status}")
            };
            log::info!("CLI[{port_name}]: {msg}");
        }
        IpcMessage::Log { level, message, .. } => {
            log::info!("CLI[{port_name}]: log[{level}] {message}");
            append_cli_log_message(port_name, &level, &message);
        }
    }
    Ok(())
}

pub async fn start(matches: &clap::ArgMatches) -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");

    // Check if config cache should be disabled (--no-config-cache flag)
    let no_cache = matches.get_flag("no-config-cache");
    persistence::set_no_cache(no_cache);

    // Check if screen capture mode is enabled
    let screen_capture_mode = matches.get_flag("debug-screen-capture");
    if screen_capture_mode {
        log::info!("📸 Screen capture mode enabled - will render once and exit");
        return run_screen_capture_mode();
    }

    // Check if debug-ci mode is enabled
    if let Some(channel_id) = matches.get_one::<String>("debug-ci") {
        log::info!(
            "🔧 Debug CI mode enabled - starting with IPC: {}",
            channel_id
        );
        return start_with_ipc(matches, channel_id).await;
    }

    // Terminal is initialized inside the rendering thread to avoid sharing
    // a Terminal instance across threads. The rendering loop will create
    // and restore the terminal on its own.

    let app = Arc::new(RwLock::new(Status::default()));

    // Initialize the global status
    self::status::init_status(app.clone())?;

    // Check if debug CI E2E test mode is enabled
    let debug_ci_e2e_enabled = matches.get_flag("debug-ci-e2e-test");
    let debug_dump_shutdown = if debug_ci_e2e_enabled {
        log::info!("🔍 Debug CI E2E test mode enabled - starting status dump thread");
        enable_debug_dump();

        let shutdown_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dump_path = std::path::PathBuf::from("/tmp/ci_tui_status.json");
        let shutdown_signal_clone = shutdown_signal.clone();

        start_status_dump_thread(dump_path, Some(shutdown_signal_clone), || {
            TuiStatus::from_global_status().and_then(|status| {
                serde_json::to_string_pretty(&status)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize TUI status: {e}"))
            })
        });

        Some(shutdown_signal)
    } else {
        None
    };

    let mut autostart_ports: Vec<String> = Vec::new();

    // Load persisted port configurations
    if let Ok(persisted_configs) = persistence::load_port_configs() {
        if !persisted_configs.is_empty() {
            let configs_vec: Vec<(String, PortConfig)> = persisted_configs.into_iter().collect();

            autostart_ports = configs_vec
                .iter()
                .filter_map(|(name, config)| match config {
                    PortConfig::Modbus { stations, .. } if !stations.is_empty() => {
                        Some(name.clone())
                    }
                    _ => None,
                })
                .collect();

            autostart_ports.sort();
            autostart_ports.dedup();

            self::status::write_status(|status| {
                for (port_name, config) in &configs_vec {
                    if !status.ports.order.contains(port_name) {
                        status.ports.order.push(port_name.clone());
                    }

                    if !status.ports.map.contains_key(port_name) {
                        let mut port_data = PortData {
                            port_name: port_name.clone(),
                            ..PortData::default()
                        };
                        port_data.port_type = "Cached".to_string();
                        status.ports.map.insert(port_name.clone(), port_data);
                    }

                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        port.config = config.clone();
                        port.config_modified = false;
                        port.state = PortState::Free;
                        port.subprocess_info = None;
                        port.status_indicator = PortStatusIndicator::NotStarted;
                        log::info!("✅ Restored cached configuration for port: {port_name}");
                    }
                }
                Ok(())
            })?;

            log::info!("📂 Restored {} port configuration(s)", configs_vec.len());
        }
    }

    if std::env::var("AOBA_TUI_FORCE_ERROR").is_ok() {
        self::status::write_status(|g| {
            ui_error_set(
                g,
                Some((
                    "demo forced error: AOBA_TUI_FORCE_ERROR".to_string(),
                    chrono::Local::now(),
                )),
            );
            Ok(())
        })?;
    }

    // Create channels for three-thread architecture
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>(); // core -> ui
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>(); // ui -> core
    let bus = Bus::new(core_rx.clone(), ui_tx.clone());

    // Thread exit/reporting channel: threads send their Result<()> here when they exit
    let (thr_tx, thr_rx) = flume::unbounded::<Result<()>>();

    // Thread 1: Core processing thread - handles UiToCore and CoreToUi communication
    // Create input kill channel before spawning core thread so core can signal input to quit
    let (input_kill_tx, input_kill_rx) = flume::bounded::<()>(1);

    let core_handle = thread::spawn({
        let core_tx = core_tx.clone();
        let thr_tx = thr_tx.clone();
        let ui_rx = ui_rx.clone();
        let input_kill_tx = input_kill_tx.clone();

        move || thr_tx.send(run_core_thread(ui_rx, core_tx, input_kill_tx))
    });

    // Thread 2: Input handling thread - processes keyboard input
    let input_handle = thread::spawn({
        let bus = bus.clone();
        move || input::run_input_thread(bus, input_kill_rx)
    });

    // Thread 3: UI rendering loop - handles rendering based on Status
    // The rendering thread will initialize and restore the terminal itself.
    let render_handle = thread::spawn(move || run_rendering_loop(bus, thr_rx));

    for port_name in &autostart_ports {
        if let Err(err) = ui_tx.send(UiToCore::ToggleRuntime(port_name.clone())) {
            log::warn!("⚠️ Failed to auto-start CLI subprocess for {port_name}: {err}");
        } else {
            log::info!("🚀 Auto-start requested for cached port {port_name}");
        }
    }

    // Rendering thread is responsible for terminal restoration; nothing to do here.

    core_handle
        .join()
        .map_err(|err| anyhow!("Failed to join core thread: {err:?}"))??;
    render_handle
        .join()
        .map_err(|err| anyhow!("Failed to join render thread: {err:?}"))??;

    input_kill_tx.send(())?;
    input_handle
        .join()
        .map_err(|err| anyhow!("Failed to join input thread: {err:?}"))??;

    // Stop debug dump thread if it was started
    if let Some(shutdown_signal) = debug_dump_shutdown {
        shutdown_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("🔍 Debug dump thread shutdown signal sent");
    }

    Ok(())
}

fn run_rendering_loop(bus: Bus, thr_rx: flume::Receiver<Result<()>>) -> Result<()> {
    // Initialize terminal inside rendering thread to avoid cross-thread Terminal usage
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    // Ensure terminal is restored on any early return
    let result = (|| {
        // Check whether any watched thread reported an error or exit
        loop {
            if let Ok(res) = thr_rx.try_recv() {
                if let Err(err) = res {
                    eprintln!("thread exited with error: {err:#}");
                    return Err(err);
                } else {
                    // thread exited successfully - treat as fatal and exit
                    log::info!("a monitored thread exited cleanly; shutting down");
                    return Ok(());
                }
            }
            // Wait for core signals with timeout
            let should_quit = match bus.core_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CoreToUi::Tick)
                | Ok(CoreToUi::Refreshed)
                | Ok(CoreToUi::Error)
                | Err(flume::RecvTimeoutError::Timeout) => {
                    // Redraw on refresh
                    false
                }
                _ => {
                    // Core thread died, exit
                    true
                }
            };

            if should_quit {
                break;
            }

            terminal.draw(|frame| {
                render_ui(frame).expect("Render failed");
            })?;
        }

        terminal.clear()?;
        Ok(())
    })();

    // Restore terminal state
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;

    // propagate inner result
    result
}

// Extracted core thread main function so it can return Result and use `?` for fallible ops.
fn run_core_thread(
    ui_rx: flume::Receiver<UiToCore>,
    core_tx: flume::Sender<CoreToUi>,
    input_kill_tx: flume::Sender<()>,
) -> Result<()> {
    let mut polling_enabled = true;
    let scan_interval = Duration::from_secs(30); // Reduced from 2s to 30s
    let mut last_scan = std::time::Instant::now() - scan_interval;
    let mut scan_in_progress = false; // Track if scan is currently running

    // do_scan extracted to module-level function below

    let _last_modbus_run = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut subprocess_manager = SubprocessManager::new();
    loop {
        // Drain UI -> core messages
        let msg_count_before = ui_rx.len();
        let mut msg_count_processed = 0;
        while let Ok(msg) = ui_rx.try_recv() {
            msg_count_processed += 1;
            let msg_name = match &msg {
                UiToCore::Quit => "Quit".to_string(),
                UiToCore::Refresh => "Refresh".to_string(),
                UiToCore::PausePolling => "PausePolling".to_string(),
                UiToCore::ResumePolling => "ResumePolling".to_string(),
                UiToCore::ToggleRuntime(port) => format!("ToggleRuntime({port})"),
                UiToCore::SendRegisterUpdate {
                    port_name,
                    station_id,
                    start_address,
                    values,
                    ..
                } => {
                    format!(
                        "SendRegisterUpdate(port={port_name}, station={station_id}, addr={start_address}, values={values:?})"
                    )
                }
            };
            log::info!("🔵 Core thread received message: {msg_name}");
            match msg {
                UiToCore::Quit => {
                    log::info!("Received quit signal");
                    log::info!("Force shutting down all CLI subprocesses before exit");
                    subprocess_manager.shutdown_all();
                    if let Err(err) = self::status::write_status(|status| {
                        for port in status.ports.map.values_mut() {
                            port.state = PortState::Free;
                            port.subprocess_info = None;
                            port.status_indicator = PortStatusIndicator::NotStarted;
                        }
                        Ok(())
                    }) {
                        log::warn!("Failed to reset port statuses while quitting: {err}");
                    }
                    // Signal input thread to quit immediately
                    if let Err(err) = input_kill_tx.send(()) {
                        log::warn!("Failed to send input kill signal: {err}");
                    }
                    // Notify UI to quit and then exit core thread
                    core_tx
                        .send(CoreToUi::Quit)
                        .map_err(|err| anyhow!("Failed to send Quit to UI core: {err}"))?;
                    return Ok(());
                }
                UiToCore::Refresh => {
                    if crate::tui::utils::scan::scan_ports(&core_tx, &mut scan_in_progress)? {
                        last_scan = std::time::Instant::now();
                    }
                }
                UiToCore::PausePolling => {
                    polling_enabled = false;
                    if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                        log::warn!("ToggleRuntime: failed to send Refreshed: {err}");
                    }
                    // Log state after refresh
                    if let Err(err) = log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::ResumePolling => {
                    polling_enabled = true;
                    core_tx.send(CoreToUi::Refreshed).map_err(|err| {
                        anyhow!("Failed to send Refreshed event to UI core: {err}")
                    })?;
                    // Log state after refresh
                    if let Err(err) = log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::ToggleRuntime(port_name) => {
                    log::info!("ToggleRuntime requested for {port_name}");

                    let subprocess_info_opt = self::status::read_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            return Ok(port.subprocess_info.clone());
                        }
                        Ok(None)
                    })?;

                    if let Some(info) = subprocess_info_opt {
                        // TUI only manages CLI subprocesses, stop it
                        if let Err(err) = subprocess_manager.stop_subprocess(&port_name) {
                            log::warn!(
                                "ToggleRuntime: failed to stop CLI subprocess for {port_name}: {err}"
                            );
                        }

                        if let Some(path) = info.data_source_path.clone() {
                            if let Err(err) = fs::remove_file(&path) {
                                log::debug!(
                                    "ToggleRuntime: failed to remove data source {path}: {err}"
                                );
                            }
                        }

                        self::status::write_status(|status| {
                            if let Some(port) = status.ports.map.get_mut(&port_name) {
                                port.state = PortState::Free;
                                port.subprocess_info = None;
                                // Port is now stopped
                                port.status_indicator =
                                    types::port::PortStatusIndicator::NotStarted;
                            }
                            Ok(())
                        })?;

                        append_subprocess_stopped_log(
                            &port_name,
                            Some(lang().tabs.log.subprocess_stopped_reason_tui.clone()),
                        );

                        if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                            log::warn!("ToggleRuntime: failed to send Refreshed: {err}");
                        }
                        if let Err(err) = log_state_snapshot() {
                            log::warn!("Failed to log state snapshot: {err}");
                        }
                        continue;
                    }

                    // Extract CLI inputs WITHOUT holding any locks during subprocess operations
                    let cli_inputs = self::status::read_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            let types::port::PortConfig::Modbus { mode, stations } = &port.config;
                            log::info!(
                                "ToggleRuntime({port_name}): checking CLI inputs - mode={}, station_count={}",
                                if mode.is_master() { "Master" } else { "Slave" },
                                stations.len()
                            );
                            if !stations.is_empty() {
                                // Use default baud rate (TUI uses CLI subprocesses, not direct port access)
                                let baud = 9600;
                                log::info!(
                                    "ToggleRuntime({port_name}): found {} station(s) - will attempt CLI subprocess",
                                    stations.len()
                                );
                                // For Master mode, pass all stations; for Slave, only first
                                return Ok(Some((mode.clone(), stations.clone(), baud)));
                            }
                            log::info!(
                                "ToggleRuntime({port_name}): no station configured - nothing to do"
                            );
                        }
                        Ok(None)
                    })?;
                    // Lock released here - safe to do long operations

                    let mut cli_started = false;

                    if let Some((mode, stations, baud_rate)) = cli_inputs {
                        match mode {
                            types::modbus::ModbusConnectionMode::Slave { .. } => {
                                // For Slave mode, use first station (slaves typically have one config)
                                let station = &stations[0];

                                log::info!(
                                    "ToggleRuntime: attempting to spawn CLI subprocess (SlavePoll) for {port_name}"
                                );

                                // Note: Slave mode polls external master, so no data source needed
                                let cli_config = CliSubprocessConfig {
                                    port_name: port_name.clone(),
                                    mode: CliMode::SlavePoll,
                                    station_id: station.station_id,
                                    register_address: station.register_address,
                                    register_length: station.register_length,
                                    register_mode: register_mode_to_cli_arg(station.register_mode)
                                        .to_string(),
                                    baud_rate,
                                    data_source: None,
                                };

                                // Spawn subprocess WITHOUT holding any status locks
                                match subprocess_manager.start_subprocess(cli_config) {
                                    Ok(()) => {
                                        if let Some(snapshot) =
                                            subprocess_manager.snapshot(&port_name)
                                        {
                                            log::info!(
                                                "ToggleRuntime: CLI subprocess spawned for {port_name} (mode={:?}, pid={:?})",
                                                snapshot.mode,
                                                snapshot.pid
                                            );
                                            let subprocess_info = PortSubprocessInfo {
                                                mode: cli_mode_to_port_mode(&snapshot.mode),
                                                ipc_socket_name: snapshot.ipc_socket_name.clone(),
                                                pid: snapshot.pid,
                                                data_source_path: None, // SlavePoll doesn't use data source
                                            };

                                            // Now update status with the result (short lock hold)
                                            self::status::write_status(|status| {
                                                if let Some(port) =
                                                    status.ports.map.get_mut(&port_name)
                                                {
                                                    port.state = PortState::OccupiedByThis;
                                                    port.subprocess_info =
                                                        Some(subprocess_info.clone());
                                                    // Port is now running
                                                    port.status_indicator = if port.config_modified
                                                    {
                                                        types::port::PortStatusIndicator::RunningWithChanges
                                                    } else {
                                                        types::port::PortStatusIndicator::Running
                                                    };
                                                }
                                                Ok(())
                                            })?;

                                            append_subprocess_spawned_log(
                                                &port_name,
                                                &snapshot.mode,
                                                snapshot.pid,
                                            );
                                            cli_started = true;

                                            // Send initial stations configuration to CLI subprocess
                                            // Retry with delays to wait for command channel to be ready
                                            log::info!(
                                                "📡 Sending initial stations configuration to CLI subprocess for {port_name}"
                                            );
                                            let mut stations_sent = false;
                                            for attempt in 1..=10 {
                                                match subprocess_manager
                                                    .send_stations_update_for_port(&port_name)
                                                {
                                                    Ok(()) => {
                                                        log::info!(
                                                            "✅ Successfully sent initial stations configuration to {port_name} (attempt {attempt})"
                                                        );
                                                        stations_sent = true;
                                                        break;
                                                    }
                                                    Err(err) if attempt < 10 => {
                                                        log::debug!(
                                                            "⏳ Attempt {attempt} to send stations update failed (command channel may not be ready yet): {err}"
                                                        );
                                                        thread::sleep(Duration::from_millis(200));
                                                    }
                                                    Err(err) => {
                                                        log::warn!(
                                                            "⚠️ Failed to send initial stations update for {port_name} after {attempt} attempts: {err}"
                                                        );
                                                    }
                                                }
                                            }
                                            if !stations_sent {
                                                log::error!(
                                                    "❌ Could not send initial stations configuration to {port_name} - CLI subprocess may not function correctly"
                                                );
                                            }
                                        } else {
                                            log::warn!(
                                                "ToggleRuntime: subprocess snapshot missing for {port_name}"
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        let err_text = err.to_string();
                                        let msg = format!(
                                            "Failed to start CLI subprocess for {port_name}: {err_text}"
                                        );
                                        append_lifecycle_log(
                                            &port_name,
                                            PortLifecyclePhase::Failed,
                                            Some(err_text.clone()),
                                        );
                                        self::status::write_status(|status| {
                                            status.temporarily.error =
                                                Some(crate::tui::status::ErrorInfo {
                                                    message: msg.clone(),
                                                    timestamp: chrono::Local::now(),
                                                });
                                            Ok(())
                                        })?;
                                        // Note: No data source file to clean up for SlavePoll mode
                                    }
                                }
                            }
                            types::modbus::ModbusConnectionMode::Master => {
                                log::info!(
                                    "ToggleRuntime: attempting to spawn CLI subprocess (MasterProvide) for {port_name} with {} station(s)",
                                    stations.len()
                                );

                                // Initialize merged data source for all stations
                                let (
                                    data_source_path,
                                    merged_station_id,
                                    merged_start_addr,
                                    merged_length,
                                ) = initialize_cli_data_source(&port_name, &stations)?;

                                let cli_config = CliSubprocessConfig {
                                    port_name: port_name.clone(),
                                    mode: CliMode::MasterProvide,
                                    station_id: merged_station_id as u8,
                                    register_address: merged_start_addr,
                                    register_length: merged_length,
                                    register_mode: register_mode_to_cli_arg(
                                        stations[0].register_mode,
                                    )
                                    .to_string(),
                                    baud_rate,
                                    data_source: Some(format!(
                                        "file:{}",
                                        data_source_path.to_string_lossy()
                                    )),
                                };

                                // Spawn subprocess WITHOUT holding any status locks
                                match subprocess_manager.start_subprocess(cli_config) {
                                    Ok(()) => {
                                        if let Some(snapshot) =
                                            subprocess_manager.snapshot(&port_name)
                                        {
                                            log::info!(
                                                "ToggleRuntime: CLI subprocess spawned for {port_name} (mode={:?}, pid={:?}, data_source={})",
                                                snapshot.mode,
                                                snapshot.pid,
                                                data_source_path.display()
                                            );
                                            let subprocess_info = PortSubprocessInfo {
                                                mode: cli_mode_to_port_mode(&snapshot.mode),
                                                ipc_socket_name: snapshot.ipc_socket_name.clone(),
                                                pid: snapshot.pid,
                                                data_source_path: Some(
                                                    data_source_path.to_string_lossy().to_string(),
                                                ),
                                            };

                                            // Now update status with the result (short lock hold)
                                            self::status::write_status(|status| {
                                                if let Some(port) =
                                                    status.ports.map.get_mut(&port_name)
                                                {
                                                    port.state = PortState::OccupiedByThis;
                                                    port.subprocess_info =
                                                        Some(subprocess_info.clone());
                                                    // Port is now running
                                                    port.status_indicator = if port.config_modified
                                                    {
                                                        types::port::PortStatusIndicator::RunningWithChanges
                                                    } else {
                                                        types::port::PortStatusIndicator::Running
                                                    };
                                                }
                                                Ok(())
                                            })?;

                                            append_subprocess_spawned_log(
                                                &port_name,
                                                &snapshot.mode,
                                                snapshot.pid,
                                            );
                                            cli_started = true;

                                            // Send initial stations configuration to CLI subprocess
                                            // Retry with delays to wait for command channel to be ready
                                            log::info!(
                                                "📡 Sending initial stations configuration to CLI subprocess for {port_name}"
                                            );
                                            let mut stations_sent = false;
                                            for attempt in 1..=10 {
                                                match subprocess_manager
                                                    .send_stations_update_for_port(&port_name)
                                                {
                                                    Ok(()) => {
                                                        log::info!(
                                                            "✅ Successfully sent initial stations configuration to {port_name} (attempt {attempt})"
                                                        );
                                                        stations_sent = true;
                                                        break;
                                                    }
                                                    Err(err) if attempt < 10 => {
                                                        log::debug!(
                                                            "⏳ Attempt {attempt} to send stations update failed (command channel may not be ready yet): {err}"
                                                        );
                                                        thread::sleep(Duration::from_millis(200));
                                                    }
                                                    Err(err) => {
                                                        log::warn!(
                                                            "⚠️ Failed to send initial stations update for {port_name} after {attempt} attempts: {err}"
                                                        );
                                                    }
                                                }
                                            }
                                            if !stations_sent {
                                                log::error!(
                                                    "❌ Could not send initial stations configuration to {port_name} - CLI subprocess may not function correctly"
                                                );
                                            }
                                        } else {
                                            log::warn!(
                                                "ToggleRuntime: subprocess snapshot missing for {port_name}"
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        let err_text = err.to_string();
                                        let msg = format!(
                                            "Failed to start CLI subprocess for {port_name}: {err_text}"
                                        );
                                        append_lifecycle_log(
                                            &port_name,
                                            PortLifecyclePhase::Failed,
                                            Some(err_text.clone()),
                                        );

                                        // Update port status indicator to show failure
                                        self::status::write_status(|status| {
                                            if let Some(port) = status.ports.map.get_mut(&port_name)
                                            {
                                                port.status_indicator = types::port::PortStatusIndicator::StartupFailed {
                                                    error_message: err_text.clone(),
                                                    timestamp: chrono::Local::now(),
                                                };
                                            }

                                            status.temporarily.error =
                                                Some(crate::tui::status::ErrorInfo {
                                                    message: msg.clone(),
                                                    timestamp: chrono::Local::now(),
                                                });
                                            Ok(())
                                        })?;

                                        if let Err(remove_err) = fs::remove_file(&data_source_path)
                                        {
                                            log::debug!(
                                                "Cleanup of data source {} failed: {remove_err}",
                                                data_source_path.to_string_lossy()
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // TUI no longer falls back to native runtime.
                    // If CLI subprocess fails to start, the port remains Free.
                    if !cli_started {
                        log::warn!(
                            "ToggleRuntime: CLI subprocess failed to start for {port_name}, port remains Free"
                        );
                    }

                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                    if let Err(err) = log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::SendRegisterUpdate {
                    port_name,
                    station_id,
                    register_type,
                    start_address,
                    values,
                } => {
                    log::info!(
                        "🔵 SendRegisterUpdate requested for {port_name}: station={station_id}, type={register_type}, addr={start_address}, values={values:?}"
                    );

                    // Per-register update forwarding removed; always send full
                    // stations update for complete synchronization.
                    log::debug!(
                        "📡 Sending full stations update for {port_name} to ensure synchronization"
                    );
                    if let Err(err) = subprocess_manager.send_stations_update_for_port(&port_name) {
                        log::warn!("❌ Failed to send stations update for {port_name}: {err}");
                    } else {
                        log::debug!("✅ Sent full stations update for {port_name}");
                    }
                }
            }
        }

        if msg_count_before > 0 || msg_count_processed > 0 {
            log::info!(
                "📊 Core thread: queue had {msg_count_before} messages, processed {msg_count_processed}",
            );
        }

        let dead_processes = subprocess_manager.reap_dead_processes();
        if !dead_processes.is_empty() {
            let mut cleanup_paths: HashMap<String, Option<String>> = HashMap::new();
            self::status::write_status(|status| {
                for (port_name, _) in &dead_processes {
                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        if port.state.is_occupied_by_this() {
                            if let Some(info) = &port.subprocess_info {
                                cleanup_paths
                                    .insert(port_name.clone(), info.data_source_path.clone());
                            }
                            port.state = PortState::Free;
                            port.subprocess_info = None;
                            // Port is now stopped
                            port.status_indicator = types::port::PortStatusIndicator::NotStarted;
                        }
                    }
                }
                Ok(())
            })?;

            for (port_name, exit_status) in dead_processes {
                if let Some(Some(path)) = cleanup_paths.remove(&port_name) {
                    if let Err(err) = fs::remove_file(&path) {
                        log::debug!("cleanup: failed to remove data source {path}: {err}");
                    }
                }

                append_subprocess_exited_log(&port_name, exit_status);

                if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                    log::warn!("Failed to send Refreshed after CLI exit for {port_name}: {err}");
                }
            }
        }

        for (port_name, message) in subprocess_manager.poll_ipc_messages() {
            if let Err(err) = handle_cli_ipc_message(port_name.as_str(), message) {
                log::warn!("Failed to handle IPC message for {port_name}: {err}");
            }
        }

        if polling_enabled
            && last_scan.elapsed() >= scan_interval
            && crate::tui::utils::scan::scan_ports(&core_tx, &mut scan_in_progress)?
        {
            last_scan = std::time::Instant::now();
        }

        // Update spinner frame for busy indicator animation
        self::status::write_status(|status| {
            status.temporarily.busy.spinner_frame =
                status.temporarily.busy.spinner_frame.wrapping_add(1);
            Ok(())
        })?;

        // Modbus communication is now handled entirely by CLI subprocesses via IPC.
        // The daemon is no longer used by TUI since we removed PortOwner::Runtime support.
        // TUI spawns CLI subprocesses which handle all port communication independently.
        // The IPC messages are polled above in poll_ipc_messages().

        core_tx
            .send(CoreToUi::Tick)
            .map_err(|err| anyhow!("failed to send Tick: {err}"))?;
        thread::sleep(Duration::from_millis(50));
    }
}

/// Render UI function that only reads from Status (immutable reference)
fn render_ui(frame: &mut Frame) -> Result<()> {
    let area = frame.area();

    let bottom_height = self::status::read_status(|status| {
        let err_lines = if status.temporarily.error.is_some() {
            1
        } else {
            0
        };
        let hints_count = match crate::tui::ui::pages::bottom_hints_for_app() {
            Ok(h) => h.len(),
            Err(_) => 0,
        };
        Ok(hints_count + err_lines)
    })?;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(3),    // main
            Constraint::Length(bottom_height as u16),
        ])
        .split(area);

    // Use the new pages module for rendering
    crate::tui::ui::title::render_title(frame, main_chunks[0])?;
    crate::tui::ui::pages::render_panels(frame, main_chunks[1])?;
    crate::tui::ui::bottom::render_bottom(frame, main_chunks[2])?;

    Ok(())
}

/// Public wrapper for render_ui for testing purposes
///
/// This allows E2E tests to render the TUI to a TestBackend without
/// spawning a real process.
pub fn render_ui_for_testing(frame: &mut Frame) -> Result<()> {
    render_ui(frame)
}

/// Start TUI in IPC mode for E2E testing
///
/// In this mode:
/// - TUI receives keyboard events via IPC from E2E tests
/// - TUI renders to TestBackend and sends screen content via IPC
/// - No real terminal is used
async fn start_with_ipc(_matches: &clap::ArgMatches, channel_id: &str) -> Result<()> {
    use ratatui::backend::TestBackend;

    log::info!(
        "🔧 Starting TUI in IPC mode with channel ID: {}",
        channel_id
    );

    // Initialize global status
    let app = Arc::new(RwLock::new(Status::default()));
    self::status::init_status(app.clone())?;

    // Create TestBackend for rendering
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;

    // Create core processing channels
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>();
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>();

    // Create dummy input kill channel (not used in IPC mode but required for run_core_thread signature)
    let (input_kill_tx, _input_kill_rx) = flume::bounded::<()>(1);

    // Start core thread
    let core_handle = thread::spawn({
        let core_tx = core_tx.clone();
        let ui_rx = ui_rx.clone();

        move || run_core_thread(ui_rx, core_tx, input_kill_tx)
    });

    // Create IPC receiver using ci_utils
    let ipc_channel_id = aoba_ci_utils::IpcChannelId(channel_id.to_string());
    log::info!("🔌 Creating IPC receiver...");
    let mut receiver = match aoba_ci_utils::IpcReceiver::new(ipc_channel_id.clone()).await {
        Ok(r) => {
            log::info!("✅ IPC receiver created successfully");
            r
        }
        Err(e) => {
            log::error!("❌ Failed to create IPC receiver: {}", e);
            return Err(e);
        }
    };

    // Create a single shared Bus instance for all key events
    let bus = Bus::new(core_rx.clone(), ui_tx.clone());

    // Main IPC loop - receive messages from E2E test
    log::info!("🔄 Starting IPC message loop");
    loop {
        match receiver.receive().await {
            Ok(aoba_ci_utils::E2EToTuiMessage::KeyPress { key }) => {
                log::info!("⌨️  Processing key press: {}", key);
                if let Ok(event) = parse_key_string(&key) {
                    if let Err(err) = crate::tui::input::handle_event(event, &bus) {
                        log::warn!("Failed to handle key event: {}", err);
                    }
                    // Small delay to allow core thread to process any messages
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
            Ok(aoba_ci_utils::E2EToTuiMessage::CharInput { ch }) => {
                log::info!("📝 Processing char input: {}", ch);
                let event = crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(ch),
                    crossterm::event::KeyModifiers::NONE,
                ));
                if let Err(err) = crate::tui::input::handle_event(event, &bus) {
                    log::warn!("Failed to handle char input: {}", err);
                }
                // Small delay to allow core thread to process any messages
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok(aoba_ci_utils::E2EToTuiMessage::RequestScreen) => {
                log::info!("🖼️  Rendering screen to TestBackend");

                // Drain any remaining core messages before rendering
                while let Ok(_msg) = bus.core_rx.try_recv() {
                    // Just consume the messages
                }

                terminal
                    .draw(|frame| {
                        if let Err(err) = render_ui(frame) {
                            log::error!("Render error: {}", err);
                        }
                    })
                    .map_err(|e| anyhow::anyhow!("Failed to draw: {}", e))?;

                let buffer = terminal.backend().buffer();
                let area = buffer.area();
                let width = area.width;
                let height = area.height;

                let mut content = String::new();
                for y in 0..height {
                    for x in 0..width {
                        let cell = &buffer[(x, y)];
                        content.push_str(cell.symbol());
                    }
                    if y < height - 1 {
                        content.push('\n');
                    }
                }

                let response = aoba_ci_utils::TuiToE2EMessage::ScreenContent {
                    content,
                    width,
                    height,
                };

                if let Err(err) = receiver.send(response).await {
                    log::error!("Failed to send screen content: {}", err);
                } else {
                    log::info!("📤 Sent screen content");
                }
            }
            Ok(aoba_ci_utils::E2EToTuiMessage::Shutdown) => {
                log::info!("🛑 Received shutdown message");
                break;
            }
            Err(err) => {
                log::error!("IPC receive error: {}", err);
                break;
            }
        }
    }

    // Cleanup
    log::info!("🧹 Cleaning up IPC mode");
    ui_tx.send(UiToCore::Quit)?;
    core_handle
        .join()
        .map_err(|err| anyhow!("Failed to join core thread: {err:?}"))??;

    Ok(())
}

/// Parse a key string into a crossterm Event
/// Supports format like "Enter", "Esc", "Up", "Down", "Left", "Right", "Char(a)", etc.
fn parse_key_string(key: &str) -> Result<crossterm::event::Event> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    let (code, modifiers) = if let Some(rest) = key.strip_prefix("Ctrl+") {
        match rest {
            "c" => (KeyCode::Char('c'), KeyModifiers::CONTROL),
            "s" => (KeyCode::Char('s'), KeyModifiers::CONTROL),
            "a" => (KeyCode::Char('a'), KeyModifiers::CONTROL),
            "Esc" => (KeyCode::Esc, KeyModifiers::CONTROL),
            "PageUp" => (KeyCode::PageUp, KeyModifiers::CONTROL),
            _ => return Err(anyhow!("Unsupported Ctrl+ combination: {}", rest)),
        }
    } else {
        match key {
            "Enter" => (KeyCode::Enter, KeyModifiers::NONE),
            "Esc" => (KeyCode::Esc, KeyModifiers::NONE),
            "Escape" => (KeyCode::Esc, KeyModifiers::NONE),
            "Backspace" => (KeyCode::Backspace, KeyModifiers::NONE),
            "Tab" => (KeyCode::Tab, KeyModifiers::NONE),
            "Up" => (KeyCode::Up, KeyModifiers::NONE),
            "Down" => (KeyCode::Down, KeyModifiers::NONE),
            "Left" => (KeyCode::Left, KeyModifiers::NONE),
            "Right" => (KeyCode::Right, KeyModifiers::NONE),
            "PageUp" => (KeyCode::PageUp, KeyModifiers::NONE),
            "PageDown" => (KeyCode::PageDown, KeyModifiers::NONE),
            "Home" => (KeyCode::Home, KeyModifiers::NONE),
            "End" => (KeyCode::End, KeyModifiers::NONE),
            _ if key.starts_with("Char(") && key.ends_with(")") => {
                let ch = key[5..key.len() - 1]
                    .chars()
                    .next()
                    .ok_or_else(|| anyhow!("Empty Char() specification"))?;
                (KeyCode::Char(ch), KeyModifiers::NONE)
            }
            _ if key.len() == 1 => {
                let ch = key.chars().next().unwrap();
                (KeyCode::Char(ch), KeyModifiers::NONE)
            }
            _ => return Err(anyhow!("Unsupported key string: {}", key)),
        }
    };

    Ok(Event::Key(KeyEvent::new(code, modifiers)))
}

/// Run screen capture mode: render UI once and exit immediately
fn run_screen_capture_mode() -> Result<()> {
    log::info!("📸 Starting screen capture mode");

    // Initialize global status
    let app = Arc::new(RwLock::new(Status::default()));
    self::status::init_status(app.clone())?;

    // Load status from /tmp/status.json if it exists
    let status_path = std::path::Path::new("/tmp/status.json");
    if status_path.exists() {
        log::info!("📄 Loading status from {}", status_path.display());
        let status_content = std::fs::read_to_string(status_path)?;
        let serializable_status: crate::tui::status::serializable::TuiStatus =
            serde_json::from_str(&status_content)?;

        // Apply the loaded status to global state
        self::status::write_status(|status| {
            serializable_status.apply_to_status(status)?;
            log::info!(
                "✅ Status loaded from file ({} ports)",
                serializable_status.ports.len()
            );
            Ok(())
        })?;
        log::info!("✅ Status loaded successfully");
    } else {
        log::warn!(
            "⚠️  No status file found at {}, using default state",
            status_path.display()
        );
    }

    // Initialize terminal
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    // Render UI once
    terminal.draw(|frame| {
        if let Err(err) = render_ui(frame) {
            log::error!("Failed to render UI: {}", err);
        }
    })?;

    // Flush to ensure content is written to the PTY
    use std::io::Write;
    io::stdout().flush()?;

    log::info!("✅ Screen rendered, waiting for termination signal...");

    // Wait for termination signal (e.g., Ctrl+C from parent process)
    // This keeps the alternate screen active so it can be captured
    use crossterm::event::{Event, KeyCode, KeyModifiers};
    loop {
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm::event::read()? {
                // Exit on Ctrl+C or Ctrl+D
                if (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
                    || (key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    break;
                }
            }
        }
    }

    // Restore terminal state
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;

    log::info!("✅ Screen capture completed successfully");
    Ok(())
}

/// Log a snapshot of the current TUI state for testing purposes.
/// This function is called after each Refreshed event to allow log-based
/// verification of state transitions.
pub fn log_state_snapshot() -> Result<()> {
    use crate::tui::status::port::PortState;
    use serde_json::json;

    self::status::read_status(|status| {
        // Extract page info
        let page_name = match &status.page {
            crate::tui::status::Page::Entry { .. } => "Entry",
            crate::tui::status::Page::ConfigPanel { .. } => "ConfigPanel",
            crate::tui::status::Page::ModbusDashboard { .. } => "ModbusDashboard",
            crate::tui::status::Page::LogPanel { .. } => "LogPanel",
            crate::tui::status::Page::About { .. } => "About",
        };

        let cursor_info = match &status.page {
            crate::tui::status::Page::Entry { cursor, .. } => {
                if let Some(c) = cursor {
                    format!("{c:?}")
                } else {
                    "None".to_string()
                }
            }
            crate::tui::status::Page::ConfigPanel { cursor, .. } => {
                format!("{cursor:?}")
            }
            crate::tui::status::Page::ModbusDashboard { cursor, .. } => {
                format!("{cursor:?}")
            }
            _ => "N/A".to_string(),
        };

        // Extract port states
        let mut port_states = vec![];
        for port_name in &status.ports.order {
            if let Some(port_arc) = status.ports.map.get(port_name) {
                let port = port_arc;
                let state_str = match &port.state {
                    PortState::Free => "Free",
                    PortState::OccupiedByThis => "OccupiedByThis",
                    PortState::OccupiedByOther => "OccupiedByOther",
                };
                port_states.push(json!({
                    "name": port_name,
                    "state": state_str,
                    "type": &port.port_type,
                }));
            }
        }

        // Extract config edit state
        let config_edit = json!({
            "active": status.temporarily.config_edit.active,
            "port": status.temporarily.config_edit.port,
            "field_index": status.temporarily.config_edit.field_index,
            "field_key": status.temporarily.config_edit.field_key,
            "buffer": status.temporarily.config_edit.buffer,
            "cursor_pos": status.temporarily.config_edit.cursor_pos,
        });

        // Build complete state snapshot
        let snapshot = json!({
            "page": page_name,
            "cursor": cursor_info,
            "ports": port_states,
            "config_edit": config_edit,
            "error": status.temporarily.error.as_ref().map(|e| &e.message),
        });

        // Log with STATE_DUMP prefix for easy parsing in tests
        log::info!("STATE_DUMP: {snapshot}");
        Ok(())
    })
}
