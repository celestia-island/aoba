use anyhow::{anyhow, Result};
use std::{
    io::Write,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use clap::ArgMatches;

use super::{
    emit_modbus_ipc_log, extract_values_from_storage, parse_register_mode, ModbusIpcLogPayload,
};
use crate::{
    api::{modbus::ModbusResponse, utils::open_serial_port},
    cli::{actions, cleanup},
    protocol::status::types::cli::OutputSink,
    utils::sleep::{sleep_1s, sleep_3s},
};

/// Outcome of a slave polling transaction.
enum SlavePollTransaction {
    Success {
        response: ModbusResponse,
    },
    Failure {
        error: anyhow::Error,
        request_frame: Vec<u8>,
    },
}

/// Execute a single slave polling transaction, returning either a successful response or
/// the failure reason along with the request frame that was sent.
fn run_slave_poll_transaction(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
) -> SlavePollTransaction {
    let mut request_frame = Vec::new();

    let transaction_result: Result<ModbusResponse> = (|| {
        log::debug!(
            "run_slave_poll_transaction: Preparing request for station={station_id}, addr=0x{register_address:04X}, len={register_length}, mode={reg_mode:?}"
        );

        let request_bytes = match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                crate::protocol::modbus::generate_pull_get_holdings_request(
                    station_id,
                    register_address,
                    register_length,
                )?
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => {
                crate::protocol::modbus::generate_pull_get_inputs_request(
                    station_id,
                    register_address,
                    register_length,
                )?
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                crate::protocol::modbus::generate_pull_get_coils_request(
                    station_id,
                    register_address,
                    register_length,
                )?
            }
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                crate::protocol::modbus::generate_pull_get_discrete_inputs_request(
                    station_id,
                    register_address,
                    register_length,
                )?
            }
        };

        request_frame = request_bytes.1.clone();

        log::info!("run_slave_poll_transaction: Sending request to master: {request_frame:02X?}");
        {
            let mut port = port_arc.lock().unwrap();
            port.write_all(&request_frame)?;
            port.flush()?;
        }

        log::debug!("run_slave_poll_transaction: Request sent, waiting for response...");

        let mut buffer = vec![0u8; 256];
        let bytes_read = {
            let mut port = port_arc.lock().unwrap();
            port.read(&mut buffer)?
        };

        if bytes_read == 0 {
            log::warn!("run_slave_poll_transaction: No response received from master");
            return Err(anyhow!("No response received"));
        }

        let response = &buffer[..bytes_read];
        log::info!(
            "run_slave_poll_transaction: Received response from master: {response:02X?} ({bytes_read} bytes)"
        );

        let values = match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding
            | crate::protocol::status::types::modbus::RegisterMode::Input => {
                if bytes_read < 5 {
                    log::error!("run_slave_poll_transaction: Response too short (need at least 5 bytes, got {bytes_read})");
                    return Err(anyhow!("Response too short"));
                }

                let byte_count = response[2] as usize;
                if bytes_read < 3 + byte_count + 2 {
                    log::error!(
                        "run_slave_poll_transaction: Incomplete response (need {} bytes, got {bytes_read})",
                        3 + byte_count + 2
                    );
                    return Err(anyhow!("Incomplete response"));
                }

                let mut values = Vec::new();
                for i in 0..(byte_count / 2) {
                    let offset = 3 + i * 2;
                    let value = u16::from_be_bytes([response[offset], response[offset + 1]]);
                    values.push(value);
                }
                values
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils
            | crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                if bytes_read < 5 {
                    log::error!("run_slave_poll_transaction: Response too short (need at least 5 bytes, got {bytes_read})");
                    return Err(anyhow!("Response too short"));
                }

                let byte_count = response[2] as usize;
                if bytes_read < 3 + byte_count + 2 {
                    log::error!(
                        "run_slave_poll_transaction: Incomplete response (need {} bytes, got {bytes_read})",
                        3 + byte_count + 2
                    );
                    return Err(anyhow!("Incomplete response"));
                }

                let mut values = Vec::new();
                for byte_idx in 0..byte_count {
                    let byte_val = response[3 + byte_idx];
                    for bit_idx in 0..8 {
                        if values.len() >= register_length as usize {
                            break;
                        }
                        let bit_value = if (byte_val & (1 << bit_idx)) != 0 {
                            1
                        } else {
                            0
                        };
                        values.push(bit_value);
                    }
                    if values.len() >= register_length as usize {
                        break;
                    }
                }
                values.truncate(register_length as usize);
                values
            }
        };

        Ok(ModbusResponse {
            station_id,
            register_address,
            register_mode: format!("{reg_mode:?}"),
            values,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    })();

    match transaction_result {
        Ok(response) => SlavePollTransaction::Success { response },
        Err(error) => SlavePollTransaction::Failure {
            error,
            request_frame,
        },
    }
}

/// Handle slave listen persist (continuous JSONL output)
pub async fn handle_slave_listen_persist(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let timeout_ms = *matches.get_one::<u32>("timeout-ms").unwrap();

    let output_sink = matches
        .get_one::<String>("output")
        .map(|s| s.parse::<OutputSink>())
        .transpose()?
        .unwrap_or(OutputSink::Stdout);

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting persistent slave listen on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate}, timeout={timeout_ms}ms)"
    );

    // Setup IPC if requested
    let mut ipc = actions::setup_ipc(matches);

    // Check if debug CI E2E test mode is enabled
    let _debug_dump_thread = if matches.get_flag("debug-ci-e2e-test") {
        log::info!("🔍 Debug CI E2E test mode enabled for CLI subprocess");

        let port_name = port.to_string();
        let station_id_copy = station_id;
        let reg_mode_copy = reg_mode;
        let register_address_copy = register_address;
        let register_length_copy = register_length;

        // Extract basename from port path (e.g., "/tmp/vcom1" -> "vcom1")
        let port_basename = std::path::Path::new(&port_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&port_name);

        let dump_path =
            std::path::PathBuf::from(format!("/tmp/ci_cli_{port_basename}_status.json"));

        Some(
            crate::protocol::status::debug_dump::start_status_dump_thread(
                dump_path,
                None,
                std::sync::Arc::new(move || {
                    crate::protocol::status::types::cli::CliStatus::new_slave_listen(
                        port_name.clone(),
                        station_id_copy,
                        reg_mode_copy,
                        register_address_copy,
                        register_length_copy,
                    )
                    .to_json()
                }),
            ),
        )
    } else {
        None
    };

    // Open serial port with configured timeout
    let port_handle =
        match open_serial_port(port, baud_rate, Duration::from_millis(timeout_ms as u64)) {
            Ok(handle) => handle,
            Err(err) => {
                if let Some(ref mut ipc_conns) = ipc {
                    let _ = ipc_conns
                        .status
                        .send(&crate::protocol::ipc::IpcMessage::PortError {
                            port_name: port.to_string(),
                            error: err.to_string(),
                            timestamp: None,
                        });
                }
                return Err(err);
            }
        };

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Notify IPC that port was opened successfully
    if let Some(ref mut ipc_conns) = ipc {
        let _ = ipc_conns
            .status
            .send(&crate::protocol::ipc::IpcMessage::PortOpened {
                port_name: port.to_string(),
                timestamp: None,
            });
        log::info!("IPC: Sent PortOpened message for {port}");
    }

    // Register cleanup to ensure port is released on program exit
    {
        let pa = port_arc.clone();
        let port_name_clone = port.to_string();
        cleanup::register_cleanup(move || {
            log::debug!("Cleanup handler: Releasing port {port_name_clone}");
            // Explicitly drop the port and wait for OS to release it
            if let Ok(mut port) = pa.lock() {
                // Try to flush any pending data
                let _ = std::io::Write::flush(&mut **port);
                log::debug!("Cleanup handler: Flushed port {port_name_clone}");
            }
            drop(pa);
            log::debug!("Cleanup handler: Port {port_name_clone} released");
        });
        log::debug!("Registered cleanup handler for port {port}");
    }

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    // Continuously listen and output JSONL
    // Track last written values to avoid duplicate consecutive outputs
    let mut last_written_values: Option<Vec<u16>> = None;

    loop {
        match listen_for_one_request(
            port_arc.clone(),
            station_id,
            register_address,
            register_length,
            reg_mode,
            storage.clone(),
        ) {
            Ok(response) => {
                let write_this = match &last_written_values {
                    Some(prev) => &response.values != prev,
                    None => true,
                };

                if write_this {
                    let json = serde_json::to_string(&response)?;
                    output_sink.write(&json)?;
                    last_written_values = Some(response.values.clone());
                }
            }
            Err(err) => {
                log::warn!("Error processing request: {err}");
                sleep_1s().await;
            }
        }
    }
}

/// Handle a single slave listen (one-shot JSON output)
pub async fn handle_slave_listen(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();

    let output_sink = matches
        .get_one::<String>("output")
        .map(|s| s.parse::<OutputSink>())
        .transpose()?
        .unwrap_or(OutputSink::Stdout);

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting slave listen on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Open serial port in a scope to ensure it's closed before returning
    let port_handle = open_serial_port(port, baud_rate, Duration::from_secs(5))?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    // Wait for one request and respond
    let response = listen_for_one_request(
        port_arc.clone(),
        station_id,
        register_address,
        register_length,
        reg_mode,
        storage,
    )?;

    // Explicitly drop port_arc to close the port
    drop(port_arc);

    // Give the OS time to fully release the port
    sleep_1s().await;

    // Output JSON to configured sink
    let json = serde_json::to_string(&response)?;
    output_sink.write(&json)?;

    Ok(())
}

/// Listen for one Modbus request and respond
fn listen_for_one_request(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
) -> Result<ModbusResponse> {
    use rmodbus::{server::ModbusFrame, ModbusProto};

    // Read request from port
    let mut buffer = vec![0u8; 256];
    let mut port = port_arc.lock().unwrap();
    let bytes_read = port.read(&mut buffer)?;
    drop(port);

    if bytes_read == 0 {
        return Err(anyhow!("No data received"));
    }

    let request = &buffer[..bytes_read];
    log::info!("Received request: {request:02X?}");

    // Parse and respond to request
    let mut response = Vec::new();
    let mut frame = ModbusFrame::new(station_id, request, ModbusProto::Rtu, &mut response);
    frame.parse()?;

    // Generate response based on register mode
    let response_bytes = match reg_mode {
        crate::protocol::status::types::modbus::RegisterMode::Holding => {
            crate::protocol::modbus::build_slave_holdings_response(
                &mut frame,
                &mut storage.lock().unwrap(),
            )?
        }
        crate::protocol::status::types::modbus::RegisterMode::Input => {
            crate::protocol::modbus::build_slave_inputs_response(
                &mut frame,
                &mut storage.lock().unwrap(),
            )?
        }
        crate::protocol::status::types::modbus::RegisterMode::Coils => {
            crate::protocol::modbus::build_slave_coils_response(
                &mut frame,
                &mut storage.lock().unwrap(),
            )?
        }
        crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
            crate::protocol::modbus::build_slave_discrete_inputs_response(
                &mut frame,
                &mut storage.lock().unwrap(),
            )?
        }
    };

    if let Some(resp) = response_bytes {
        // Send response
        let mut port = port_arc.lock().unwrap();
        port.write_all(&resp)?;
        port.flush()?;
        log::info!("Sent response: {resp:02X?}");
    }

    // Extract values from storage for response
    let values =
        extract_values_from_storage(&storage, register_address, register_length, reg_mode)?;

    Ok(ModbusResponse {
        station_id,
        register_address,
        register_mode: format!("{reg_mode:?}"),
        values,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Handle slave poll (act as Modbus Master/Client - send request and wait for response)
pub async fn handle_slave_poll(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let timeout_ms = *matches.get_one::<u32>("timeout-ms").unwrap();

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting slave poll on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate}, timeout={timeout_ms}ms)"
    );

    let response = {
        // Open serial port in a scope to ensure it's closed before returning
        let port_handle =
            open_serial_port(port, baud_rate, Duration::from_millis(timeout_ms as u64))?;

        let port_arc = Arc::new(Mutex::new(port_handle));

        // Execute single poll transaction
        let outcome = run_slave_poll_transaction(
            port_arc.clone(),
            station_id,
            register_address,
            register_length,
            reg_mode,
        );

        // Explicitly drop port_arc to close the port
        drop(port_arc);
        match outcome {
            SlavePollTransaction::Success { response, .. } => response,
            SlavePollTransaction::Failure { error, .. } => return Err(error),
        }
    };

    // Output response as JSON
    let json = serde_json::to_string(&response)?;
    println!("{json}");

    Ok(())
}

/// Handle slave poll persist (continuous polling mode)
pub async fn handle_slave_poll_persist(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let request_interval_ms = *matches.get_one::<u32>("request-interval-ms").unwrap();
    let timeout_ms = *matches.get_one::<u32>("timeout-ms").unwrap();

    let output_sink = matches
        .get_one::<String>("output")
        .map(|s| s.parse::<OutputSink>())
        .transpose()?
        .unwrap_or(OutputSink::Stdout);

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting persistent slave poll on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate}, request_interval={request_interval_ms}ms, timeout={timeout_ms}ms)"
    );

    // Setup IPC if requested
    let mut ipc = actions::setup_ipc(matches);

    // Check if debug CI E2E test mode is enabled
    let _debug_dump_thread = if matches.get_flag("debug-ci-e2e-test") {
        log::info!("🔍 Debug CI E2E test mode enabled for CLI subprocess");

        let port_name = port.to_string();
        let station_id_copy = station_id;
        let reg_mode_copy = reg_mode;
        let register_address_copy = register_address;
        let register_length_copy = register_length;

        // Sanitize port name for filename
        // Extract basename from port path (e.g., "/tmp/vcom1" -> "vcom1")
        let port_basename = std::path::Path::new(&port_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&port_name);

        let dump_path =
            std::path::PathBuf::from(format!("/tmp/ci_cli_{port_basename}_status.json"));

        Some(
            crate::protocol::status::debug_dump::start_status_dump_thread(
                dump_path,
                None,
                std::sync::Arc::new(move || {
                    crate::protocol::status::types::cli::CliStatus::new_slave_poll(
                        port_name.clone(),
                        station_id_copy,
                        reg_mode_copy,
                        register_address_copy,
                        register_length_copy,
                    )
                    .to_json()
                }),
            ),
        )
    } else {
        None
    };

    // Open serial port with configured timeout
    let port_handle =
        match open_serial_port(port, baud_rate, Duration::from_millis(timeout_ms as u64)) {
            Ok(handle) => handle,
            Err(err) => {
                if let Some(ref mut ipc_conns) = ipc {
                    let _ = ipc_conns
                        .status
                        .send(&crate::protocol::ipc::IpcMessage::PortError {
                            port_name: port.to_string(),
                            error: err.to_string(),
                            timestamp: None,
                        });
                }
                return Err(err);
            }
        };

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Notify IPC that port was opened successfully
    if let Some(ref mut ipc_conns) = ipc {
        let _ = ipc_conns
            .status
            .send(&crate::protocol::ipc::IpcMessage::PortOpened {
                port_name: port.to_string(),
                timestamp: None,
            });
        log::info!("IPC: Sent PortOpened message for {port}");
    }

    // Register cleanup to ensure port is released on program exit
    {
        let pa = port_arc.clone();
        let port_name_clone = port.to_string();
        cleanup::register_cleanup(move || {
            log::debug!("Cleanup handler: Releasing port {port_name_clone}");
            // Explicitly drop the port and wait for OS to release it
            if let Ok(mut port) = pa.lock() {
                // Try to flush any pending data
                let _ = std::io::Write::flush(&mut **port);
                log::debug!("Cleanup handler: Flushed port {port_name_clone}");
            }
            drop(pa);
            log::debug!("Cleanup handler: Port {port_name_clone} released");
        });
        log::debug!("Registered cleanup handler for port {port}");
    }

    // Flag to track whether command channel has been accepted
    static COMMAND_ACCEPTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    // Continuously poll
    // Keep track of last written values to avoid consecutive duplicate outputs
    let mut last_written_values: Option<Vec<u16>> = None;
    let mut last_failure_log: Option<(String, Instant)> = None;

    // Store current station configuration (can be updated via IPC)
    let mut current_station_id = station_id;
    let mut current_register_address = register_address;
    let mut current_register_length = register_length;
    let mut current_reg_mode = reg_mode;

    // Track pending write requests: (register_address, value, register_type)
    let mut pending_writes: std::collections::VecDeque<(u16, u16, String)> =
        std::collections::VecDeque::new();

    loop {
        // Try to accept incoming command channel connection (non-blocking)
        if let Some(ref mut ipc_conns) = ipc {
            if !COMMAND_ACCEPTED.load(std::sync::atomic::Ordering::Relaxed) {
                match ipc_conns.command_listener.accept() {
                    Ok(()) => {
                        log::info!("Command channel accepted");
                        COMMAND_ACCEPTED.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    Err(e) => {
                        // Don't log every attempt to avoid spam, just keep trying
                        log::trace!("Command channel accept not ready yet: {e}");
                    }
                }
            }
        }

        // Check for incoming StationsUpdate commands for multi-station support
        if COMMAND_ACCEPTED.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(ref mut ipc_conns) = ipc {
                if let Ok(Some(msg)) = ipc_conns.command_listener.try_recv() {
                    match msg {
                        crate::protocol::ipc::IpcMessage::StationsUpdate {
                            stations_data,
                            update_reason,
                            ..
                        } => {
                            log::info!(
                                "Received stations update ({} bytes), reason={:?}",
                                stations_data.len(),
                                update_reason
                            );

                            // Deserialize and use the stations configuration
                            if let Ok(stations) = postcard::from_bytes::<
                                Vec<crate::cli::config::StationConfig>,
                            >(&stations_data)
                            {
                                log::info!("Deserialized {} stations", stations.len());

                                // For slave poll mode, we focus on the first slave station
                                // In the future, this could be extended to handle multiple stations
                                if let Some(first_station) = stations.first() {
                                    if first_station.mode == crate::cli::config::StationMode::Slave
                                    {
                                        // Update configuration from the first slave station
                                        current_station_id = first_station.station_id;

                                        // Determine write behavior based on update reason
                                        // - "user_edit": Write all values including 0 (user intention)
                                        // - "initial_config": Skip all writes (configuration initialization)
                                        // - "sync" or None: Skip 0 values (likely defaults/heartbeat)
                                        // - "read_response": Always apply (actual modbus read data)
                                        let should_process_writes = match update_reason.as_deref() {
                                            Some("user_edit") => true,     // User explicitly edited, write everything
                                            Some("read_response") => true, // Actual read data, always apply
                                            Some("initial_config") => false, // Initial config, skip writes
                                            Some("sync") | None => last_written_values.is_some(), // Sync/unknown: only if we've read before
                                            Some(other) => {
                                                log::warn!("Unknown update_reason: {other}, defaulting to sync behavior");
                                                last_written_values.is_some()
                                            }
                                        };

                                        let allow_zero_writes =
                                            matches!(update_reason.as_deref(), Some("user_edit"));

                                        log::info!(
                                            "Write decision: reason={:?}, should_process={}, allow_zeros={}",
                                            update_reason,
                                            should_process_writes,
                                            allow_zero_writes
                                        );

                                        if should_process_writes {
                                            // Check all register ranges for values that should be written
                                            for range in &first_station.map.holding {
                                                for (idx, &value) in
                                                    range.initial_values.iter().enumerate()
                                                {
                                                    let addr = range.address_start + idx as u16;

                                                    // Skip zero values unless this is a user edit
                                                    if value == 0 && !allow_zero_writes {
                                                        log::debug!("⏭️ Skipping write for holding register 0x{addr:04X} (value=0, reason={:?})", update_reason);
                                                        continue;
                                                    }

                                                    let needs_write = if let Some(prev_vals) =
                                                        &last_written_values
                                                    {
                                                        let relative_idx = (addr
                                                            - current_register_address)
                                                            as usize;
                                                        relative_idx >= prev_vals.len()
                                                            || prev_vals[relative_idx] != value
                                                    } else {
                                                        false // Should not happen since we checked above
                                                    };

                                                    if needs_write {
                                                        log::info!("📤 Queueing write for holding register 0x{addr:04X} = 0x{value:04X}");
                                                        pending_writes.push_back((
                                                            addr,
                                                            value,
                                                            "holding".to_string(),
                                                        ));
                                                    }
                                                }
                                            }

                                            for range in &first_station.map.coils {
                                                for (idx, &value) in
                                                    range.initial_values.iter().enumerate()
                                                {
                                                    let addr = range.address_start + idx as u16;

                                                    // Skip zero values unless this is a user edit
                                                    if value == 0 && !allow_zero_writes {
                                                        log::debug!("⏭️ Skipping write for coil 0x{addr:04X} (value=0, reason={:?})", update_reason);
                                                        continue;
                                                    }

                                                    let needs_write = if let Some(prev_vals) =
                                                        &last_written_values
                                                    {
                                                        let relative_idx = (addr
                                                            - current_register_address)
                                                            as usize;
                                                        relative_idx >= prev_vals.len()
                                                            || prev_vals[relative_idx] != value
                                                    } else {
                                                        false
                                                    };

                                                    if needs_write {
                                                        log::info!("📤 Queueing write for coil 0x{addr:04X} = 0x{value:04X}");
                                                        pending_writes.push_back((
                                                            addr,
                                                            value,
                                                            "coil".to_string(),
                                                        ));
                                                    }
                                                }
                                            }
                                        } else {
                                            log::info!("⏸️  Skipping initial write queue - will sync after first read from master");
                                        }

                                        // Find the first register range to use as the polling target
                                        if let Some(range) = first_station.map.holding.first() {
                                            current_register_address = range.address_start;
                                            current_register_length = range.length;
                                            current_reg_mode = crate::protocol::status::types::modbus::RegisterMode::Holding;
                                            log::info!(
                                                "Updated slave config: station={current_station_id}, type=Holding, addr=0x{current_register_address:04X}, len={current_register_length}"
                                            );
                                        } else if let Some(range) = first_station.map.coils.first()
                                        {
                                            current_register_address = range.address_start;
                                            current_register_length = range.length;
                                            current_reg_mode = crate::protocol::status::types::modbus::RegisterMode::Coils;
                                            log::info!(
                                                "Updated slave config: station={current_station_id}, type=Coils, addr=0x{current_register_address:04X}, len={current_register_length}"
                                            );
                                        } else if let Some(range) =
                                            first_station.map.discrete_inputs.first()
                                        {
                                            current_register_address = range.address_start;
                                            current_register_length = range.length;
                                            current_reg_mode = crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs;
                                            log::info!(
                                                "Updated slave config: station={current_station_id}, type=DiscreteInputs, addr=0x{current_register_address:04X}, len={current_register_length}"
                                            );
                                        } else if let Some(range) = first_station.map.input.first()
                                        {
                                            current_register_address = range.address_start;
                                            current_register_length = range.length;
                                            current_reg_mode = crate::protocol::status::types::modbus::RegisterMode::Input;
                                            log::info!(
                                                "Updated slave config: station={current_station_id}, type=Input, addr=0x{current_register_address:04X}, len={current_register_length}"
                                            );
                                        }

                                        // Don't reset last_written_values here - we need it to detect changes
                                    } else {
                                        log::warn!("Received master station config in slave poll mode, ignoring");
                                    }
                                } else {
                                    log::warn!("Received empty stations list, keeping current configuration");
                                }
                            } else {
                                log::warn!("Failed to deserialize stations data");
                            }
                        }
                        _ => {
                            log::debug!("Received unexpected IPC message in slave poll mode");
                        }
                    }
                }
            }
        }

        // Process pending write requests before read polling
        if let Some((write_addr, write_value, write_type)) = pending_writes.pop_front() {
            log::info!("📤 Processing pending write: addr=0x{write_addr:04X}, value=0x{write_value:04X}, type={write_type}");

            let write_result = if write_type == "holding" {
                // Send write request for holding register
                match crate::protocol::modbus::generate_pull_set_holding_request(
                    current_station_id,
                    write_addr,
                    write_value,
                ) {
                    Ok((mut request, raw_frame)) => {
                        log::info!("Generated write request frame: {raw_frame:02X?}");

                        // Send request
                        {
                            let mut port = port_arc.lock().unwrap();
                            if let Err(e) = port.write_all(&raw_frame) {
                                Err(anyhow!("Failed to write request: {e}"))
                            } else if let Err(e) = port.flush() {
                                Err(anyhow!("Failed to flush: {e}"))
                            } else {
                                Ok(())
                            }
                        }?;

                        // Wait for response
                        let mut buffer = vec![0u8; 256];
                        let bytes_read = {
                            let mut port = port_arc.lock().unwrap();
                            port.read(&mut buffer)?
                        };

                        if bytes_read == 0 {
                            Err(anyhow!("No response received for write"))
                        } else {
                            let response = &buffer[..bytes_read];
                            log::info!("Received write response: {response:02X?}");

                            // Parse response
                            match crate::protocol::modbus::parse_pull_set_response(
                                &mut request,
                                response.to_vec(),
                            ) {
                                Ok(()) => {
                                    log::info!("✅ Write successful for 0x{write_addr:04X} = 0x{write_value:04X}");
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                    }
                    Err(e) => Err(e),
                }
            } else if write_type == "coil" {
                // TODO: Implement coil write using set_coils_bulk
                log::warn!("⚠️ Coil write not yet implemented, skipping");
                Ok(()) // Pretend success for now
            } else {
                Err(anyhow!("Unsupported write type: {write_type}"))
            };

            // Send write completion message via IPC
            if let Some(ref mut ipc_conns) = ipc {
                let msg = crate::protocol::ipc::IpcMessage::register_write_complete(
                    port.to_string(),
                    current_station_id,
                    write_addr,
                    write_value,
                    write_type.clone(),
                    write_result.is_ok(),
                    write_result.as_ref().err().map(|e| e.to_string()),
                );

                if let Err(e) = ipc_conns.status.send(&msg) {
                    log::warn!("Failed to send RegisterWriteComplete via IPC: {e}");
                }
            }

            // Continue to next iteration to process more writes if any
            continue;
        }

        match run_slave_poll_transaction(
            port_arc.clone(),
            current_station_id,
            current_register_address,
            current_register_length,
            current_reg_mode,
        ) {
            SlavePollTransaction::Success { response } => {
                let write_this = match &last_written_values {
                    Some(prev) => &response.values != prev,
                    None => true,
                };

                if write_this {
                    let json = serde_json::to_string(&response)?;
                    output_sink.write(&json)?;
                    last_written_values = Some(response.values.clone());

                    if let Some(ref mut ipc_conns) = ipc {
                        log::info!(
                            "IPC: Sending StationsUpdate for {port}: station={current_station_id}, type={:?}, addr=0x{current_register_address:04X}, values={:?}",
                            current_reg_mode,
                            response.values
                        );

                        let station_config = crate::cli::config::StationConfig::single_range(
                            current_station_id,
                            crate::cli::config::StationMode::Slave,
                            current_reg_mode,
                            current_register_address,
                            current_register_length,
                            Some(response.values.clone()),
                        );

                        match postcard::to_allocvec(&vec![station_config]) {
                            Ok(stations_data) => {
                                let msg = crate::protocol::ipc::IpcMessage::stations_update(
                                    stations_data,
                                );
                                if let Err(e) = ipc_conns.status.send(&msg) {
                                    log::warn!("Failed to send StationsUpdate via IPC: {e}");
                                } else {
                                    log::debug!("Successfully sent StationsUpdate via IPC");
                                }
                            }
                            Err(e) => {
                                log::warn!("Failed to serialize StationConfig: {e}");
                            }
                        }
                    }
                }

                last_failure_log = None;

                // Wait configured request interval after successful poll
                if request_interval_ms < 1000 {
                    sleep_1s().await;
                } else {
                    sleep_3s().await;
                }
            }
            SlavePollTransaction::Failure {
                request_frame,
                error,
            } => {
                let error_text = format!("{error:#}");
                log::warn!("Poll error: {error_text}");

                let trimmed_error = error_text.trim().to_string();
                let should_emit = !matches!(
                    &last_failure_log,
                    Some((prev, ts)) if prev == &trimmed_error && ts.elapsed() < Duration::from_secs(2)
                );

                if should_emit {
                    emit_modbus_ipc_log(
                        &mut ipc,
                        ModbusIpcLogPayload {
                            port,
                            direction: "tx",
                            frame: &request_frame,
                            station_id: Some(current_station_id),
                            register_mode: Some(current_reg_mode),
                            start_address: Some(current_register_address),
                            quantity: Some(current_register_length),
                            success: Some(false),
                            error: Some(trimmed_error.clone()),
                            config_index: None,
                        },
                    );
                    last_failure_log = Some((trimmed_error, Instant::now()));
                }

                // Wait configured timeout duration after failure
                if timeout_ms < 1000 {
                    sleep_1s().await;
                } else {
                    sleep_3s().await;
                }
            }
        }
    }
}

/// Handle IPC channel mode for slave-listen-persist
/// Creates a Unix socket server that accepts connections and responds to JSON requests
/// Each connection is handled in a separate async task to support multiple clients
pub async fn handle_slave_listen_ipc_channel(
    matches: &ArgMatches,
    port: &str,
    ipc_socket_path: &str,
) -> Result<()> {
    use interprocess::local_socket::{prelude::*, ListenerOptions};

    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let timeout_ms = *matches.get_one::<u32>("timeout-ms").unwrap();

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting IPC channel slave listen on {port} (socket={ipc_socket_path}, station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate}, timeout={timeout_ms}ms)"
    );

    // Setup IPC if requested
    let mut ipc = actions::setup_ipc(matches);

    // Open serial port with configured timeout
    let port_handle =
        match open_serial_port(port, baud_rate, Duration::from_millis(timeout_ms as u64)) {
            Ok(handle) => handle,
            Err(err) => {
                if let Some(ref mut ipc_conns) = ipc {
                    let _ = ipc_conns
                        .status
                        .send(&crate::protocol::ipc::IpcMessage::PortError {
                            port_name: port.to_string(),
                            error: err.to_string(),
                            timestamp: None,
                        });
                }
                return Err(err);
            }
        };

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Notify IPC that port was opened successfully
    if let Some(ref mut ipc_conns) = ipc {
        let _ = ipc_conns
            .status
            .send(&crate::protocol::ipc::IpcMessage::PortOpened {
                port_name: port.to_string(),
                timestamp: None,
            });
        log::info!("IPC: Sent PortOpened message for {port}");
    }

    // Register cleanup to ensure port is released on program exit
    {
        let pa = port_arc.clone();
        let port_name_clone = port.to_string();
        cleanup::register_cleanup(move || {
            log::debug!("Cleanup handler: Releasing port {port_name_clone}");
            if let Ok(mut port) = pa.lock() {
                let _ = std::io::Write::flush(&mut **port);
                log::debug!("Cleanup handler: Flushed port {port_name_clone}");
            }
            drop(pa);
            log::debug!("Cleanup handler: Port {port_name_clone} released");
        });
        log::debug!("Registered cleanup handler for port {port}");
    }

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    // Create IPC Unix socket listener
    log::info!("Creating IPC Unix socket listener at {ipc_socket_path}");

    // Remove existing socket file if it exists (Unix only)
    #[cfg(unix)]
    {
        if std::path::Path::new(ipc_socket_path).exists() {
            log::warn!("Removing existing socket file: {ipc_socket_path}");
            let _ = std::fs::remove_file(ipc_socket_path);
        }
    }

    let listener =
        match ipc_socket_path.to_ns_name::<interprocess::local_socket::GenericNamespaced>() {
            Ok(name) => ListenerOptions::new().name(name).create_sync(),
            Err(_) => {
                // Fall back to file path
                let path =
                    ipc_socket_path.to_fs_name::<interprocess::local_socket::GenericFilePath>()?;
                ListenerOptions::new().name(path).create_sync()
            }
        }?;

    log::info!("IPC socket listener created, waiting for connections...");

    // Spawn a task to handle incoming connections
    let connection_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    loop {
        // Accept incoming connection (blocking)
        let stream = match listener.accept() {
            Ok(stream) => stream,
            Err(e) => {
                log::error!("Failed to accept connection: {e}");
                sleep_1s().await;
                continue;
            }
        };

        let conn_id = connection_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        log::info!("Accepted IPC connection #{conn_id}");

        // Clone resources for the connection handler
        let port_arc_clone = port_arc.clone();
        let storage_clone = storage.clone();
        // Build context to avoid too many arguments
        let ctx = IpcConnectionContext {
            port_arc: port_arc_clone,
            station_id,
            register_address,
            register_length,
            reg_mode,
            storage: storage_clone,
        };

        // Spawn a task to handle this connection
        crate::core::task_manager::spawn_task(async move {
            if let Err(e) = handle_ipc_connection(stream, conn_id, ctx.clone()).await {
                log::error!("Connection #{conn_id} error: {e}");
            }
            log::info!("Connection #{conn_id} closed");
            Ok(())
        });
    }
}

/// Handle a single IPC connection (half-duplex JSON request-response)
#[derive(Clone)]
struct IpcConnectionContext {
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
}

async fn handle_ipc_connection(
    mut stream: interprocess::local_socket::Stream,
    conn_id: usize,
    ctx: IpcConnectionContext,
) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};

    log::info!("Connection #{conn_id}: Ready to receive JSON requests");

    // Use BufReader for line-based reading
    let mut reader = BufReader::new(&mut stream);

    loop {
        // Read one line (JSON request)
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            // Connection closed
            log::info!("Connection #{conn_id}: Client closed connection");
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        log::info!("Connection #{conn_id}: Received request: {line}");

        // Parse JSON request (we don't actually use it, just validate it's valid JSON)
        let _request: serde_json::Value = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(e) => {
                let error_response = serde_json::json!({
                    "success": false,
                    "error": format!("Invalid JSON: {e}")
                });
                let response_str = serde_json::to_string(&error_response)?;
                // Get mutable access to stream to write
                let stream_ref = reader.get_mut();
                writeln!(stream_ref, "{response_str}")?;
                stream_ref.flush()?;
                continue;
            }
        };

        // Process request and generate response
        let response = match listen_for_one_request(
            ctx.port_arc.clone(),
            ctx.station_id,
            ctx.register_address,
            ctx.register_length,
            ctx.reg_mode,
            ctx.storage.clone(),
        ) {
            Ok(modbus_response) => {
                serde_json::json!({
                    "success": true,
                    "data": modbus_response
                })
            }
            Err(e) => {
                serde_json::json!({
                    "success": false,
                    "error": format!("{e}")
                })
            }
        };

        // Send response (JSON line)
        let response_str = serde_json::to_string(&response)?;
        log::info!("Connection #{conn_id}: Sending response: {response_str}");
        // Get mutable access to stream to write
        let stream_ref = reader.get_mut();
        writeln!(stream_ref, "{response_str}")?;
        stream_ref.flush()?;
    }

    Ok(())
}
