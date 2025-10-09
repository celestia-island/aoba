use anyhow::{anyhow, Result};
use std::{
    io::{BufRead, BufReader},
    sync::{Arc, Mutex},
    time::Duration,
};

use clap::ArgMatches;
use rmodbus::{server::context::ModbusContext, ModbusProto};

use super::{parse_data_line, parse_register_mode, DataSource, ModbusResponse};
use crate::cli::cleanup;
use crate::protocol::modbus::{build_slave_coils_response, build_slave_holdings_response};

/// Handle master provide (temporary: output once and exit)
pub fn handle_master_provide(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let data_source_str = matches
        .get_one::<String>("data-source")
        .ok_or_else(|| anyhow!("--data-source is required for master mode"))?;

    let reg_mode = parse_register_mode(register_mode)?;
    let data_source = data_source_str.parse::<DataSource>()?;

    log::info!(
        "Starting master provide on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Read one line of data
    let values = read_one_data_update(&data_source)?;

    // Initialize modbus storage with values
    use rmodbus::server::storage::ModbusStorageSmall;
    let storage = Arc::new(Mutex::new(ModbusStorageSmall::default()));
    {
        let mut context = storage.lock().unwrap();
        match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                for (i, &val) in values.iter().enumerate() {
                    context.set_holding(register_address + i as u16, val)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                for (i, &val) in values.iter().enumerate() {
                    context.set_coil(register_address + i as u16, val != 0)?;
                }
            }
            _ => {
                return Err(anyhow!(
                    "Master provide only supports holding registers and coils"
                ));
            }
        }
    }

    // Open serial port and wait for one request, then respond and exit
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_secs(5))
        .open()
        .map_err(|e| anyhow!("Failed to open port {port}: {e}"))?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Wait for request and respond once
    let mut buffer = [0u8; 256];
    let mut assembling: Vec<u8> = Vec::new();
    let frame_gap = Duration::from_millis(10);
    let start_time = std::time::Instant::now();

    loop {
        if start_time.elapsed() > Duration::from_secs(10) {
            return Err(anyhow!("Timeout waiting for request"));
        }

        let mut port = port_arc.lock().unwrap();
        match port.read(&mut buffer) {
            Ok(n) if n > 0 => {
                assembling.extend_from_slice(&buffer[..n]);
                std::thread::sleep(frame_gap);
            }
            Ok(_) => {
                if !assembling.is_empty() {
                    // Frame complete - process it
                    drop(port);

                    let request = assembling.clone();
                    let response =
                        respond_to_request(port_arc.clone(), &request, station_id, &storage)?;

                    // Output JSON
                    let json = serde_json::to_string(&response)?;
                    println!("{json}");

                    // Explicitly drop port_arc to close the port
                    drop(port_arc);
                    std::thread::sleep(Duration::from_millis(100));

                    return Ok(());
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if !assembling.is_empty() {
                    // Frame complete - process it
                    drop(port);

                    let request = assembling.clone();
                    let response =
                        respond_to_request(port_arc.clone(), &request, station_id, &storage)?;

                    // Output JSON
                    let json = serde_json::to_string(&response)?;
                    println!("{json}");

                    // Explicitly drop port_arc to close the port
                    drop(port_arc);
                    std::thread::sleep(Duration::from_millis(100));

                    return Ok(());
                }
            }
            Err(e) => {
                return Err(anyhow!("Error reading from port: {e}"));
            }
        }
    }
}

/// Handle master provide persist (continuous JSONL output)
/// Master mode acts as Modbus Slave/Server - listens for requests and responds with data
pub fn handle_master_provide_persist(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let data_source_str = matches
        .get_one::<String>("data-source")
        .ok_or_else(|| anyhow!("--data-source is required for master mode"))?;

    let reg_mode = parse_register_mode(register_mode)?;
    let data_source = data_source_str.parse::<DataSource>()?;

    log::info!(
        "Starting persistent master provide on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );
    log::info!("Master mode: acting as Modbus Slave/Server - listening for requests and responding with data");

    // Open serial port with longer timeout for reading requests
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|e| anyhow!("Failed to open port {port}: {e}"))?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Register cleanup to ensure port is released on program exit
    {
        let pa = port_arc.clone();
        cleanup::register_cleanup(move || {
            // Drop the Arc to release the port and give OS time
            drop(pa);
            std::thread::sleep(Duration::from_millis(100));
        });
    }

    // Initialize modbus storage with values from data source
    use rmodbus::server::storage::ModbusStorageSmall;
    let storage = Arc::new(Mutex::new(ModbusStorageSmall::default()));

    // Load initial data into storage
    let initial_values = read_one_data_update(&data_source)?;
    log::info!("Loaded initial values: {initial_values:?}");
    {
        let mut context = storage.lock().unwrap();
        match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                for (i, &val) in initial_values.iter().enumerate() {
                    context.set_holding(register_address + i as u16, val)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                for (i, &val) in initial_values.iter().enumerate() {
                    context.set_coil(register_address + i as u16, val != 0)?;
                }
            }
            _ => {
                return Err(anyhow!(
                    "Master provide only supports holding registers and coils"
                ));
            }
        }
    }

    // Start a background thread to update storage with new values from data source
    let storage_clone = storage.clone();
    let data_source_clone = data_source.clone();
    let update_thread = std::thread::spawn(move || {
        update_storage_loop(storage_clone, data_source_clone, reg_mode, register_address)
    });

    // Main loop: listen for requests and respond
    let mut buffer = [0u8; 256];
    let mut assembling: Vec<u8> = Vec::new();
    let mut last_byte_time: Option<std::time::Instant> = None;
    let frame_gap = Duration::from_millis(10); // Inter-frame gap

    log::info!("CLI Master: Entering main loop, listening for requests on {port}");

    loop {
        // Check if update thread has panicked
        if update_thread.is_finished() {
            return Err(anyhow!("Data update thread terminated unexpectedly"));
        }

        let mut port = port_arc.lock().unwrap();
        match port.read(&mut buffer) {
            Ok(n) if n > 0 => {
                log::info!(
                    "CLI Master: Read {n} bytes from port: {:02X?}",
                    &buffer[..n]
                );
                assembling.extend_from_slice(&buffer[..n]);
                last_byte_time = Some(std::time::Instant::now());
            }
            Ok(_) => {
                // No data available, check if we have a complete frame
                if !assembling.is_empty() {
                    if let Some(last_time) = last_byte_time {
                        if last_time.elapsed() >= frame_gap {
                            // Frame complete - process it
                            log::info!(
                                "CLI Master: Frame complete ({} bytes), processing request",
                                assembling.len()
                            );
                            drop(port); // Release port lock before processing

                            let request = assembling.clone();
                            assembling.clear();
                            last_byte_time = None;

                            // Process the request and generate response
                            match respond_to_request(
                                port_arc.clone(),
                                &request,
                                station_id,
                                &storage,
                            ) {
                                Ok(response) => {
                                    let json = serde_json::to_string(&response)?;
                                    println!("{json}");
                                }
                                Err(e) => {
                                    log::warn!("Error responding to request: {e}");
                                }
                            }

                            continue; // Re-acquire lock in next iteration
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Timeout is normal, check for frame completion
                if !assembling.is_empty() {
                    if let Some(last_time) = last_byte_time {
                        if last_time.elapsed() >= frame_gap {
                            // Frame complete - process it
                            drop(port); // Release port lock before processing

                            let request = assembling.clone();
                            assembling.clear();
                            last_byte_time = None;

                            // Process the request and generate response
                            match respond_to_request(
                                port_arc.clone(),
                                &request,
                                station_id,
                                &storage,
                            ) {
                                Ok(response) => {
                                    let json = serde_json::to_string(&response)?;
                                    println!("{json}");
                                }
                                Err(e) => {
                                    log::warn!("Error responding to request: {e}");
                                }
                            }

                            continue; // Re-acquire lock in next iteration
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("Error reading from port: {e}");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        drop(port);

        // Small sleep to avoid busy loop
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Respond to a Modbus request (acting as Slave/Server)
fn respond_to_request(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    request: &[u8],
    station_id: u8,
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
) -> Result<ModbusResponse> {
    use rmodbus::server::ModbusFrame;

    if request.len() < 2 {
        return Err(anyhow!("Request too short"));
    }

    let request_station_id = request[0];
    if request_station_id != station_id {
        log::debug!(
            "Ignoring request for station {request_station_id} (we are station {station_id})",
        );
        return Err(anyhow!(
            "Request for different station ID: {request_station_id} (we are {station_id})",
        ));
    }

    log::info!("Received request: {request:02X?}");

    // Parse and respond to request
    let mut context = storage.lock().unwrap();
    let mut response_buf = Vec::new();
    let mut frame = ModbusFrame::new(station_id, request, ModbusProto::Rtu, &mut response_buf);
    frame.parse()?;

    let response = match frame.func {
        rmodbus::consts::ModbusFunction::GetHoldings => {
            match build_slave_holdings_response(&mut frame, &mut context) {
                Ok(Some(resp)) => resp,
                _ => return Err(anyhow!("Failed to build holdings response")),
            }
        }
        rmodbus::consts::ModbusFunction::GetCoils => {
            match build_slave_coils_response(&mut frame, &mut context) {
                Ok(Some(resp)) => resp,
                _ => return Err(anyhow!("Failed to build coils response")),
            }
        }
        _ => {
            return Err(anyhow!("Unsupported function code: {:?}", frame.func));
        }
    };

    drop(context);

    // Send response
    let mut port = port_arc.lock().unwrap();
    port.write_all(&response)?;
    port.flush()?;
    drop(port);

    log::info!("Sent response: {response:02X?}");

    // Extract values from response for JSON output
    let values = extract_values_from_response(&response)?;

    Ok(ModbusResponse {
        station_id,
        register_address: 0, // Would need to parse from request
        register_mode: format!("{:?}", frame.func),
        values,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Update storage loop - continuously reads data from source and updates storage
fn update_storage_loop(
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    data_source: DataSource,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    register_address: u16,
) -> Result<()> {
    loop {
        match &data_source {
            DataSource::File(path) => {
                let file = std::fs::File::open(path)?;
                let reader = BufReader::new(file);

                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }

                    match parse_data_line(&line) {
                        Ok(values) => {
                            log::info!("Updating storage with values: {values:?}");
                            let mut context = storage.lock().unwrap();
                            match reg_mode {
                                crate::protocol::status::types::modbus::RegisterMode::Holding => {
                                    for (i, &val) in values.iter().enumerate() {
                                        context.set_holding(register_address + i as u16, val)?;
                                    }
                                }
                                crate::protocol::status::types::modbus::RegisterMode::Coils => {
                                    for (i, &val) in values.iter().enumerate() {
                                        context.set_coil(register_address + i as u16, val != 0)?;
                                    }
                                }
                                _ => {}
                            }
                            drop(context);

                            // Wait a bit before next update to avoid overwhelming
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(e) => {
                            log::warn!("Error parsing data line: {e}");
                        }
                    }
                }

                // After reading all lines, loop back to start of file
                log::debug!("Reached end of data file, looping back to start");
            }
            DataSource::Pipe(path) => {
                // Open named pipe (FIFO) and continuously read from it
                let file = std::fs::File::open(path)?;
                let reader = BufReader::new(file);

                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }

                    match parse_data_line(&line) {
                        Ok(values) => {
                            log::info!("Updating storage with values: {values:?}");
                            let mut context = storage.lock().unwrap();
                            match reg_mode {
                                crate::protocol::status::types::modbus::RegisterMode::Holding => {
                                    for (i, &val) in values.iter().enumerate() {
                                        context.set_holding(register_address + i as u16, val)?;
                                    }
                                }
                                crate::protocol::status::types::modbus::RegisterMode::Coils => {
                                    for (i, &val) in values.iter().enumerate() {
                                        context.set_coil(register_address + i as u16, val != 0)?;
                                    }
                                }
                                _ => {}
                            }
                            drop(context);

                            // Wait a bit before next update
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(e) => {
                            log::warn!("Error parsing data line: {e}");
                        }
                    }
                }

                // Pipe closed by writer, reopen and continue
                log::debug!("Pipe closed, reopening...");
            }
        }
    }
}

/// Extract values from a Modbus response frame
fn extract_values_from_response(response: &[u8]) -> Result<Vec<u16>> {
    if response.len() < 3 {
        return Ok(vec![]);
    }

    let _station_id = response[0];
    let function_code = response[1];
    let byte_count = response[2] as usize;

    match function_code {
        0x03 => {
            // Read Holding Registers response
            if response.len() < 3 + byte_count {
                return Err(anyhow!("Response too short for holdings"));
            }
            let mut values = Vec::new();
            for i in (0..byte_count).step_by(2) {
                if 3 + i + 1 < response.len() {
                    let val = u16::from_be_bytes([response[3 + i], response[3 + i + 1]]);
                    values.push(val);
                }
            }
            Ok(values)
        }
        0x01 => {
            // Read Coils response
            if response.len() < 3 + byte_count {
                return Err(anyhow!("Response too short for coils"));
            }
            let mut values = Vec::new();
            for byte_idx in 0..byte_count {
                let byte = response[3 + byte_idx];
                for bit_idx in 0..8 {
                    let coil_val = if (byte & (1 << bit_idx)) != 0 { 1 } else { 0 };
                    values.push(coil_val);
                }
            }
            Ok(values)
        }
        _ => Ok(vec![]),
    }
}

/// Read one data update from source
fn read_one_data_update(source: &DataSource) -> Result<Vec<u16>> {
    match source {
        DataSource::File(path) => {
            let file = std::fs::File::open(path)?;
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            reader.read_line(&mut line)?;
            parse_data_line(&line)
        }
        DataSource::Pipe(path) => {
            // Open named pipe (FIFO) for reading
            let file = std::fs::File::open(path)?;
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            reader.read_line(&mut line)?;
            parse_data_line(&line)
        }
    }
}
