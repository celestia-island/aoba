use anyhow::{anyhow, Result};
use clap::ArgMatches;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{extract_values_from_storage, parse_register_mode, ModbusResponse, OutputSink};
use crate::{actions, cleanup};

/// Handle slave listen (temporary: output once and exit)
pub fn handle_slave_listen(matches: &ArgMatches, port: &str) -> Result<()> {
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

    let response = {
        // Open serial port in a scope to ensure it's closed before returning
        let port_handle = serialport::new(port, baud_rate)
            .timeout(Duration::from_secs(5))
            .open()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))?;

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
        std::thread::sleep(Duration::from_millis(100));

        response
    };

    // Output JSON to configured sink
    let json = serde_json::to_string(&response)?;
    output_sink.write(&json)?;

    Ok(())
}

/// Handle slave listen persist (continuous JSONL output)
pub fn handle_slave_listen_persist(matches: &ArgMatches, port: &str) -> Result<()> {
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
        "Starting persistent slave listen on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
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

        Some(aoba_protocol::status::debug_dump::start_status_dump_thread(
            dump_path,
            None,
            move || {
                aoba_protocol::status::types::cli::CliStatus::new_slave_listen(
                    port_name.clone(),
                    station_id_copy,
                    reg_mode_copy,
                    register_address_copy,
                    register_length_copy,
                )
                .to_json()
            },
        ))
    } else {
        None
    };

    // Open serial port
    let port_handle = match serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
    {
        Ok(handle) => handle,
        Err(err) => {
            // Try to send error via IPC if available
            if let Some(ref mut ipc_conns) = ipc {
                let _ = ipc_conns
                    .status
                    .send(&aoba_protocol::ipc::IpcMessage::PortError {
                        port_name: port.to_string(),
                        error: format!("Failed to open port: {err}"),
                        timestamp: None,
                    });
            }
            return Err(anyhow!("Failed to open port {port}: {err}"));
        }
    };
    let port_arc = Arc::new(Mutex::new(port_handle));

    // Notify IPC that port was opened successfully
    if let Some(ref mut ipc_conns) = ipc {
        let _ = ipc_conns
            .status
            .send(&aoba_protocol::ipc::IpcMessage::PortOpened {
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
            // Give the OS time to fully release the file descriptor
            std::thread::sleep(Duration::from_millis(200));
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
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Listen for one Modbus request and respond
fn listen_for_one_request(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: aoba_protocol::status::types::modbus::RegisterMode,
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
        aoba_protocol::status::types::modbus::RegisterMode::Holding => {
            aoba_protocol::modbus::build_slave_holdings_response(
                &mut frame,
                &mut storage.lock().unwrap(),
            )?
        }
        aoba_protocol::status::types::modbus::RegisterMode::Input => {
            aoba_protocol::modbus::build_slave_inputs_response(
                &mut frame,
                &mut storage.lock().unwrap(),
            )?
        }
        aoba_protocol::status::types::modbus::RegisterMode::Coils => {
            aoba_protocol::modbus::build_slave_coils_response(
                &mut frame,
                &mut storage.lock().unwrap(),
            )?
        }
        aoba_protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
            aoba_protocol::modbus::build_slave_discrete_inputs_response(
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
pub fn handle_slave_poll(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting slave poll on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    let response = {
        // Open serial port in a scope to ensure it's closed before returning
        let port_handle = serialport::new(port, baud_rate)
            .timeout(Duration::from_secs(5))
            .open()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))?;

        let port_arc = Arc::new(Mutex::new(port_handle));

        // Send request and wait for response
        let response = send_request_and_wait(
            port_arc.clone(),
            station_id,
            register_address,
            register_length,
            reg_mode,
        )?;

        // Explicitly drop port_arc to close the port
        drop(port_arc);
        response
    };

    // Output response as JSON
    let json = serde_json::to_string_pretty(&response)?;
    println!("{json}");

    Ok(())
}

/// Send a Modbus request and wait for response (act as Modbus Master/Client)
fn send_request_and_wait(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: aoba_protocol::status::types::modbus::RegisterMode,
) -> Result<ModbusResponse> {
    log::debug!(
        "send_request_and_wait: Preparing request for station={station_id}, addr=0x{register_address:04X}, len={register_length}, mode={reg_mode:?}"
    );

    // Generate request based on register mode
    let request_bytes = match reg_mode {
        aoba_protocol::status::types::modbus::RegisterMode::Holding => {
            aoba_protocol::modbus::generate_pull_get_holdings_request(
                station_id,
                register_address,
                register_length,
            )?
        }
        aoba_protocol::status::types::modbus::RegisterMode::Input => {
            aoba_protocol::modbus::generate_pull_get_inputs_request(
                station_id,
                register_address,
                register_length,
            )?
        }
        aoba_protocol::status::types::modbus::RegisterMode::Coils => {
            aoba_protocol::modbus::generate_pull_get_coils_request(
                station_id,
                register_address,
                register_length,
            )?
        }
        aoba_protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
            aoba_protocol::modbus::generate_pull_get_discrete_inputs_request(
                station_id,
                register_address,
                register_length,
            )?
        }
    };

    // Send request
    log::info!(
        "send_request_and_wait: Sending request to master: {:02X?}",
        request_bytes.1
    );
    let mut port = port_arc.lock().unwrap();
    port.write_all(&request_bytes.1)?; // .1 is the raw frame bytes
    port.flush()?;
    log::debug!("send_request_and_wait: Request sent, waiting for response...");
    drop(port);

    // Wait for response
    let mut buffer = vec![0u8; 256];
    let mut port = port_arc.lock().unwrap();
    let bytes_read = port.read(&mut buffer)?;
    drop(port);

    if bytes_read == 0 {
        log::warn!("send_request_and_wait: No response received from master");
        return Err(anyhow!("No response received"));
    }

    let response = &buffer[..bytes_read];
    log::info!(
        "send_request_and_wait: Received response from master: {response:02X?} ({bytes_read} bytes)"
    );

    // Parse response
    let values = match reg_mode {
        aoba_protocol::status::types::modbus::RegisterMode::Holding
        | aoba_protocol::status::types::modbus::RegisterMode::Input => {
            // Response format for read holdings/inputs:
            // [slave_id, function_code, byte_count, data..., crc_low, crc_high]
            if bytes_read < 5 {
                log::error!("send_request_and_wait: Response too short (need at least 5 bytes, got {bytes_read})");
                return Err(anyhow!("Response too short"));
            }

            let byte_count = response[2] as usize;
            log::debug!(
                "send_request_and_wait: Parsing response - byte_count={byte_count}, expected data bytes={byte_count}"
            );
            if bytes_read < 3 + byte_count + 2 {
                log::error!(
                    "send_request_and_wait: Incomplete response (need {} bytes, got {bytes_read})",
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
            log::info!(
                "send_request_and_wait: Parsed {} register values: {values:?}",
                values.len(),
            );
            values
        }
        aoba_protocol::status::types::modbus::RegisterMode::Coils
        | aoba_protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
            // Response format for read coils/discrete inputs:
            // [slave_id, function_code, byte_count, data..., crc_low, crc_high]
            if bytes_read < 5 {
                log::error!("send_request_and_wait: Response too short (need at least 5 bytes, got {bytes_read})");
                return Err(anyhow!("Response too short"));
            }

            let byte_count = response[2] as usize;
            if bytes_read < 3 + byte_count + 2 {
                log::error!(
                    "send_request_and_wait: Incomplete response (need {} bytes, got {bytes_read})",
                    3 + byte_count + 2
                );
                return Err(anyhow!("Incomplete response"));
            }

            let mut values = Vec::new();
            // Each byte contains 8 bits (coils/discrete inputs)
            for byte_idx in 0..byte_count {
                let byte_val = response[3 + byte_idx];
                for bit_idx in 0..8 {
                    if values.len() >= register_length as usize {
                        break;
                    }
                    // Extract bit value (LSB first)
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
            // Truncate to requested length
            values.truncate(register_length as usize);
            log::info!(
                "send_request_and_wait: Parsed {} coil/discrete values: {values:?}",
                values.len(),
            );
            values
        }
    };

    log::info!(
        "send_request_and_wait: Successfully completed - station={station_id}, addr=0x{register_address:04X}, values={values:?}"
    );

    Ok(ModbusResponse {
        station_id,
        register_address,
        register_mode: format!("{reg_mode:?}"),
        values,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Handle slave poll persist (continuous polling mode)
pub fn handle_slave_poll_persist(matches: &ArgMatches, port: &str) -> Result<()> {
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
        "Starting persistent slave poll on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Setup IPC if requested
    let mut ipc = actions::setup_ipc(matches);
    
    // Create shared storage for multiple stations configuration
    // Initialize with the station from command line args
    let initial_station = crate::config::StationConfig {
        station_id: station_id as u16,
        mode: reg_mode,
        map: crate::config::RegisterMap {
            holding: if matches!(reg_mode, crate::config::RegisterMode::Holding) {
                vec![crate::config::RegisterRange {
                    address_start: register_address,
                    initial_values: vec![0; register_length as usize],
                }]
            } else {
                vec![]
            },
            input: if matches!(reg_mode, crate::config::RegisterMode::Input) {
                vec![crate::config::RegisterRange {
                    address_start: register_address,
                    initial_values: vec![0; register_length as usize],
                }]
            } else {
                vec![]
            },
            coils: if matches!(reg_mode, crate::config::RegisterMode::Coils) {
                vec![crate::config::RegisterRange {
                    address_start: register_address,
                    initial_values: vec![0; register_length as usize],
                }]
            } else {
                vec![]
            },
            discrete_inputs: if matches!(reg_mode, crate::config::RegisterMode::DiscreteInputs) {
                vec![crate::config::RegisterRange {
                    address_start: register_address,
                    initial_values: vec![0; register_length as usize],
                }]
            } else {
                vec![]
            },
        },
    };
    let stations = Arc::new(Mutex::new(vec![initial_station]));

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

        Some(aoba_protocol::status::debug_dump::start_status_dump_thread(
            dump_path,
            None,
            move || {
                aoba_protocol::status::types::cli::CliStatus::new_slave_poll(
                    port_name.clone(),
                    station_id_copy,
                    reg_mode_copy,
                    register_address_copy,
                    register_length_copy,
                )
                .to_json()
            },
        ))
    } else {
        None
    };

    // Open serial port
    let port_handle = match serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(500))
        .open()
    {
        Ok(handle) => handle,
        Err(err) => {
            // Try to send error via IPC if available
            if let Some(ref mut ipc_conns) = ipc {
                let _ = ipc_conns
                    .status
                    .send(&aoba_protocol::ipc::IpcMessage::PortError {
                        port_name: port.to_string(),
                        error: format!("Failed to open port: {err}"),
                        timestamp: None,
                    });
            }
            return Err(anyhow!("Failed to open port {port}: {err}"));
        }
    };

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Notify IPC that port was opened successfully
    if let Some(ref mut ipc_conns) = ipc {
        let _ = ipc_conns
            .status
            .send(&aoba_protocol::ipc::IpcMessage::PortOpened {
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
            // Give the OS time to fully release the file descriptor
            std::thread::sleep(Duration::from_millis(200));
            log::debug!("Cleanup handler: Port {port_name_clone} released");
        });
        log::debug!("Registered cleanup handler for port {port}");
    }

    // Flag to track whether command channel has been accepted
    static COMMAND_ACCEPTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    // Continuously poll
    // Keep track of last written values per station to avoid consecutive duplicate outputs
    let mut last_written_values: std::collections::HashMap<(u8, u16, crate::config::RegisterMode), Vec<u16>> = 
        std::collections::HashMap::new();

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

        // Check for incoming StationsUpdate commands
        if COMMAND_ACCEPTED.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(ref mut ipc_conns) = ipc {
                if let Ok(Some(msg)) = ipc_conns.command_listener.try_recv() {
                    match msg {
                        aoba_protocol::ipc::IpcMessage::StationsUpdate {
                            stations_data, ..
                        } => {
                            log::info!("Received stations update, {} bytes", stations_data.len());

                            // Deserialize stations using postcard
                            if let Ok(new_stations) = postcard::from_bytes::<
                                Vec<crate::config::StationConfig>,
                            >(&stations_data)
                            {
                                log::info!("Deserialized {} stations", new_stations.len());
                                
                                // Update the stations configuration
                                if let Ok(mut stations_guard) = stations.lock() {
                                    *stations_guard = new_stations;
                                    log::info!("Updated stations configuration with {} stations", stations_guard.len());
                                } else {
                                    log::warn!("Failed to acquire lock on stations for update");
                                }
                            } else {
                                log::warn!("Failed to deserialize stations update");
                            }
                        }
                        _ => {
                            log::debug!("Received unexpected IPC message in slave poll mode");
                        }
                    }
                }
            }
        }

        // Poll all configured stations
        let stations_snapshot = if let Ok(guard) = stations.lock() {
            guard.clone()
        } else {
            log::warn!("Failed to acquire lock on stations for polling");
            std::thread::sleep(Duration::from_millis(500));
            continue;
        };

        for station in &stations_snapshot {
            let station_id = station.station_id as u8;
            
            // Poll each register range in the station
            for range in &station.map.holding {
                if let Ok(response) = send_request_and_wait(
                    port_arc.clone(),
                    station_id,
                    range.address_start,
                    range.initial_values.len() as u16,
                    crate::config::RegisterMode::Holding,
                ) {
                    let key = (station_id, range.address_start, crate::config::RegisterMode::Holding);
                    let write_this = last_written_values.get(&key)
                        .map(|prev| &response.values != prev)
                        .unwrap_or(true);

                    if write_this {
                        let json = serde_json::to_string(&response).ok();
                        if let Some(json_str) = json {
                            let _ = output_sink.write(&json_str);
                        }
                        last_written_values.insert(key, response.values);
                    }
                }
            }

            for range in &station.map.input {
                if let Ok(response) = send_request_and_wait(
                    port_arc.clone(),
                    station_id,
                    range.address_start,
                    range.initial_values.len() as u16,
                    crate::config::RegisterMode::Input,
                ) {
                    let key = (station_id, range.address_start, crate::config::RegisterMode::Input);
                    let write_this = last_written_values.get(&key)
                        .map(|prev| &response.values != prev)
                        .unwrap_or(true);

                    if write_this {
                        let json = serde_json::to_string(&response).ok();
                        if let Some(json_str) = json {
                            let _ = output_sink.write(&json_str);
                        }
                        last_written_values.insert(key, response.values);
                    }
                }
            }

            for range in &station.map.coils {
                if let Ok(response) = send_request_and_wait(
                    port_arc.clone(),
                    station_id,
                    range.address_start,
                    range.initial_values.len() as u16,
                    crate::config::RegisterMode::Coils,
                ) {
                    let key = (station_id, range.address_start, crate::config::RegisterMode::Coils);
                    let write_this = last_written_values.get(&key)
                        .map(|prev| &response.values != prev)
                        .unwrap_or(true);

                    if write_this {
                        let json = serde_json::to_string(&response).ok();
                        if let Some(json_str) = json {
                            let _ = output_sink.write(&json_str);
                        }
                        last_written_values.insert(key, response.values);
                    }
                }
            }

            for range in &station.map.discrete_inputs {
                if let Ok(response) = send_request_and_wait(
                    port_arc.clone(),
                    station_id,
                    range.address_start,
                    range.initial_values.len() as u16,
                    crate::config::RegisterMode::DiscreteInputs,
                ) {
                    let key = (station_id, range.address_start, crate::config::RegisterMode::DiscreteInputs);
                    let write_this = last_written_values.get(&key)
                        .map(|prev| &response.values != prev)
                        .unwrap_or(true);

                    if write_this {
                        let json = serde_json::to_string(&response).ok();
                        if let Some(json_str) = json {
                            let _ = output_sink.write(&json_str);
                        }
                        last_written_values.insert(key, response.values);
                    }
                }
            }
        }

        // Small delay between poll cycles
        std::thread::sleep(Duration::from_millis(500));
    }
}
