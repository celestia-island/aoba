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

        Some(
            crate::protocol::status::debug_dump::start_status_dump_thread(
                dump_path,
                None,
                move || {
                    crate::protocol::status::types::cli::CliStatus::new_slave_listen(
                        port_name.clone(),
                        station_id_copy,
                        reg_mode_copy,
                        register_address_copy,
                        register_length_copy,
                    )
                    .to_json()
                },
            ),
        )
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
                    .send(&crate::protocol::ipc::IpcMessage::PortError {
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
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
) -> Result<ModbusResponse> {
    log::debug!(
        "send_request_and_wait: Preparing request for station={station_id}, addr=0x{register_address:04X}, len={register_length}, mode={reg_mode:?}"
    );

    // Generate request based on register mode
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
        crate::protocol::status::types::modbus::RegisterMode::Holding
        | crate::protocol::status::types::modbus::RegisterMode::Input => {
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
        crate::protocol::status::types::modbus::RegisterMode::Coils
        | crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
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
                move || {
                    crate::protocol::status::types::cli::CliStatus::new_slave_poll(
                        port_name.clone(),
                        station_id_copy,
                        reg_mode_copy,
                        register_address_copy,
                        register_length_copy,
                    )
                    .to_json()
                },
            ),
        )
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
                    .send(&crate::protocol::ipc::IpcMessage::PortError {
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
            // Give the OS time to fully release the file descriptor
            std::thread::sleep(Duration::from_millis(200));
            log::debug!("Cleanup handler: Port {port_name_clone} released");
        });
        log::debug!("Registered cleanup handler for port {port}");
    }

    // Continuously poll
    // Keep track of last written values to avoid consecutive duplicate outputs
    let mut last_written_values: Option<Vec<u16>> = None;

    loop {
        match send_request_and_wait(
            port_arc.clone(),
            station_id,
            register_address,
            register_length,
            reg_mode,
        ) {
            Ok(response) => {
                // If the values are identical to the last written ones, skip writing
                let write_this = match &last_written_values {
                    Some(prev) => &response.values != prev,
                    None => true,
                };

                if write_this {
                    let json = serde_json::to_string(&response)?;
                    output_sink.write(&json)?;
                    last_written_values = Some(response.values.clone());

                    // Send RegisterUpdate via IPC
                    if let Some(ref _ipc_conns) = ipc {
                        log::info!(
                            "IPC: Would send StationsUpdate for {port}: station={station_id}, type={register_mode}, addr=0x{register_address:04X}, values={:?}",
                            response.values
                        );
                        // TODO: With new design, we send full StationsUpdate instead of individual RegisterUpdate
                        // For now, we skip this to avoid breaking the new IPC message format
                        // Later, we'll implement proper state synchronization that sends all stations
                    }
                }
            }
            Err(err) => {
                log::warn!("Poll error: {err}");
            }
        }

        // Small delay between polls
        std::thread::sleep(Duration::from_millis(500));
    }
}
