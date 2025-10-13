use anyhow::{anyhow, Result};
use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::Hasher,
    io::{BufRead, BufReader},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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
        .map_err(|err| anyhow!("Failed to open port {port}: {err}"))?;

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
            Err(err) => {
                return Err(anyhow!("Error reading from port: {err}"));
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

    // Setup IPC if requested
    let mut ipc_connections = crate::cli::actions::setup_ipc(matches);

    // Open serial port with longer timeout for reading requests
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|err| {
            if let Some(ref mut ipc_conns) = ipc_connections {
                let _ = ipc_conns
                    .status
                    .send(&crate::protocol::ipc::IpcMessage::PortError {
                        port_name: port.to_string(),
                        error: format!("Failed to open port: {err}"),
                        timestamp: None,
                    });
            }
            anyhow!("Failed to open port {port}: {err}")
        })?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Notify IPC that port was opened successfully
    if let Some(ref mut ipc_conns) = ipc_connections {
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
    // For pipe data sources we spawn the background updater; for file data sources
    // the updater will still be spawned but printing of JSON to stdout is
    // de-duplicated below to avoid repeated identical log lines when polled
    let storage_clone = storage.clone();
    let data_source_clone = data_source.clone();

    // Track recent changed ranges so the main loop can bypass debounce when a
    // request overlaps a recently-updated register range.
    let changed_ranges: Arc<Mutex<Vec<(u16, u16, Instant)>>> = Arc::new(Mutex::new(Vec::new()));
    let changed_ranges_clone = changed_ranges.clone();
    let update_thread = std::thread::spawn(move || {
        update_storage_loop(
            storage_clone,
            data_source_clone,
            reg_mode,
            register_address,
            changed_ranges_clone,
        )
    });

    // Parse optional debounce seconds argument (floating seconds). Default 1.0s
    // Single-precision seconds argument
    let debounce_seconds = matches
        .get_one::<f32>("debounce-seconds")
        .copied()
        .unwrap_or(1.0_f32);

    // Printing/de-duplication state
    // We track by a key derived from the request bytes + response values to
    // handle two duplicate scenarios:
    // 1) The same request arrives multiple times in a short window -> debounce
    // 2) Different requests produce the same values -> dedupe by values
    // Use RefCell for interior mutability so the closure doesn't capture a
    // long-lived mutable borrow of these maps and block other borrows.
    let last_print_times: RefCell<HashMap<u64, Instant>> = RefCell::new(HashMap::new());
    let last_values_by_key: RefCell<HashMap<u64, Vec<u16>>> = RefCell::new(HashMap::new());

    // Debounce window: if same request key printed within this duration, skip
    // Convert floating seconds to Duration (support fractional seconds)
    let debounce_window = if debounce_seconds <= 0.0 {
        Duration::from_secs(0)
    } else {
        let ms = (debounce_seconds * 1000.0).round() as u64;
        Duration::from_millis(ms)
    };

    // TTL for stale cache entries (so the maps don't grow forever). Use a
    // multiple of debounce_window; if debounce_window is zero, use 10s default.
    let cache_ttl = if debounce_window == Duration::from_secs(0) {
        Duration::from_secs(10)
    } else {
        debounce_window * 4
    };

    // Helper to optionally print response JSON while handling duplicate suppression
    // Uses a key (hash) which should be derived from the original request bytes
    // so repeated identical requests within the debounce window won't spam stdout.
    let print_response = |request_key: u64, response: &ModbusResponse, force: bool| -> Result<()> {
        let now = Instant::now();

        // If force flag is set, bypass debounce and emit immediately
        if force {
            let json = serde_json::to_string(response)?;
            println!("{json}");
            last_values_by_key
                .borrow_mut()
                .insert(request_key, response.values.clone());
            last_print_times.borrow_mut().insert(request_key, now);
            return Ok(());
        }

        // If values are identical to last printed for this key, skip
        if let Some(prev_vals) = last_values_by_key.borrow().get(&request_key) {
            if prev_vals == &response.values {
                // Update last print time to extend debounce even if we don't print
                last_print_times.borrow_mut().insert(request_key, now);
                return Ok(());
            }
        }

        // If we printed something for this key recently, skip printing (debounce)
        if let Some(last) = last_print_times.borrow().get(&request_key) {
            if now.duration_since(*last) < debounce_window {
                // Update stored values and time, but do not emit
                last_values_by_key
                    .borrow_mut()
                    .insert(request_key, response.values.clone());
                last_print_times.borrow_mut().insert(request_key, now);
                return Ok(());
            }
        }

        // Otherwise emit JSON and record time/values
        let json = serde_json::to_string(response)?;
        println!("{json}");
        last_values_by_key
            .borrow_mut()
            .insert(request_key, response.values.clone());
        last_print_times.borrow_mut().insert(request_key, now);
        Ok(())
    };

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

        // Accept command connection if not yet connected
        if let Some(ref mut ipc_conns) = ipc_connections {
            // Try to accept command connection (non-blocking after first attempt)
            // This is a one-time operation
            static COMMAND_ACCEPTED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !COMMAND_ACCEPTED.load(std::sync::atomic::Ordering::Relaxed) {
                if let Err(e) = ipc_conns.command_listener.accept() {
                    log::debug!("Command channel accept not ready: {e}");
                } else {
                    log::info!("Command channel accepted");
                    COMMAND_ACCEPTED.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }

            // Check for incoming commands
            if let Ok(Some(msg)) = ipc_conns.command_listener.try_recv() {
                match msg {
                    crate::protocol::ipc::IpcMessage::ConfigUpdate {
                        station_id: new_station_id,
                        register_type,
                        start_address: new_start_address,
                        register_length: new_length,
                        ..
                    } => {
                        log::info!("Received config update: station={new_station_id}, type={register_type}, addr={new_start_address}, len={new_length}");
                        // TODO: Apply configuration updates to runtime
                        // For now, just log them
                    }
                    crate::protocol::ipc::IpcMessage::RegisterUpdate {
                        station_id: _,
                        register_type,
                        start_address: update_start_addr,
                        values,
                        ..
                    } => {
                        log::info!("Received register update: type={register_type}, addr={update_start_addr}, values={values:?}");
                        // Apply register updates directly to storage
                        let mut context = storage.lock().unwrap();
                        if register_type == "holding" {
                            for (i, &val) in values.iter().enumerate() {
                                if let Err(e) =
                                    context.set_holding(update_start_addr + i as u16, val)
                                {
                                    log::warn!("Failed to set holding register: {e}");
                                }
                            }
                            log::info!("Applied register update to storage");
                        }
                    }
                    _ => {
                        log::debug!("Ignoring non-command IPC message");
                    }
                }
            }
        }

        // Cleanup stale entries from the print caches on each loop iteration
        // to prevent unbounded growth. We remove entries older than cache_ttl.
        if !last_print_times.borrow().is_empty() {
            let now = Instant::now();
            // Collect expired keys first (avoid holding an immutable borrow while mutating)
            let expired: Vec<u64> = last_print_times
                .borrow()
                .iter()
                .filter_map(|(k, &t)| {
                    if now.duration_since(t) > cache_ttl {
                        Some(*k)
                    } else {
                        None
                    }
                })
                .collect();
            for k in expired {
                last_print_times.borrow_mut().remove(&k);
                last_values_by_key.borrow_mut().remove(&k);
            }
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
                            // Try to parse request range from raw bytes (func at index 1)
                            let parsed_range = if request.len() >= 8 {
                                let func = request[1];
                                match func {
                                    0x01 => {
                                        let start = u16::from_be_bytes([request[2], request[3]]);
                                        let qty = u16::from_be_bytes([request[4], request[5]]);
                                        Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Coils))
                                    }
                                    0x03 => {
                                        let start = u16::from_be_bytes([request[2], request[3]]);
                                        let qty = u16::from_be_bytes([request[4], request[5]]);
                                        Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Holding))
                                    }
                                    _ => None,
                                }
                            } else {
                                None
                            };

                            match respond_to_request(
                                port_arc.clone(),
                                &request,
                                station_id,
                                &storage,
                            ) {
                                Ok(response) => {
                                    let mut hasher = DefaultHasher::new();
                                    hasher.write(&request);
                                    let request_key = hasher.finish();

                                    // Determine overlap with recent changes
                                    let mut force = false;
                                    if let Some((start, qty, _mode)) = parsed_range {
                                        let now = Instant::now();
                                        let cr = changed_ranges.lock().unwrap();
                                        for (cstart, clen, t) in cr.iter() {
                                            if now.duration_since(*t) > cache_ttl {
                                                continue;
                                            }
                                            let a1 = start as u32;
                                            let a2 = (start + qty) as u32;
                                            let b1 = *cstart as u32;
                                            let b2 = (cstart + clen) as u32;
                                            if a1 < b2 && b1 < a2 {
                                                force = true;
                                                break;
                                            }
                                        }
                                    }

                                    if let Err(e) = print_response(request_key, &response, force) {
                                        log::warn!("Failed to print response: {e}");
                                    }
                                }
                                Err(err) => {
                                    log::warn!("Error responding to request: {err}");
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
                            // Try to parse request range from raw bytes (func at index 1)
                            let parsed_range = if request.len() >= 8 {
                                let func = request[1];
                                match func {
                                    0x01 => {
                                        let start = u16::from_be_bytes([request[2], request[3]]);
                                        let qty = u16::from_be_bytes([request[4], request[5]]);
                                        Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Coils))
                                    }
                                    0x03 => {
                                        let start = u16::from_be_bytes([request[2], request[3]]);
                                        let qty = u16::from_be_bytes([request[4], request[5]]);
                                        Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Holding))
                                    }
                                    _ => None,
                                }
                            } else {
                                None
                            };

                            match respond_to_request(
                                port_arc.clone(),
                                &request,
                                station_id,
                                &storage,
                            ) {
                                Ok(response) => {
                                    // Build a stable key from the request bytes to use for debounce
                                    let mut hasher = DefaultHasher::new();
                                    hasher.write(&request);
                                    let request_key = hasher.finish();

                                    // Determine overlap with recent changes
                                    let mut force = false;
                                    if let Some((start, qty, _mode)) = parsed_range {
                                        let now = Instant::now();
                                        let cr = changed_ranges.lock().unwrap();
                                        for (cstart, clen, t) in cr.iter() {
                                            if now.duration_since(*t) > cache_ttl {
                                                continue;
                                            }
                                            let a1 = start as u32;
                                            let a2 = (start + qty) as u32;
                                            let b1 = *cstart as u32;
                                            let b2 = (cstart + clen) as u32;
                                            if a1 < b2 && b1 < a2 {
                                                force = true;
                                                break;
                                            }
                                        }
                                    }

                                    if let Err(e) = print_response(request_key, &response, force) {
                                        log::warn!("Failed to print response: {e}");
                                    }
                                }
                                Err(err) => {
                                    log::warn!("Error responding to request: {err}");
                                }
                            }

                            continue; // Re-acquire lock in next iteration
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!("Error reading from port: {err}");
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
        log::warn!(
            "respond_to_request: Request too short (len={})",
            request.len()
        );
        return Err(anyhow!("Request too short"));
    }

    let request_station_id = request[0];
    if request_station_id != station_id {
        log::debug!(
            "respond_to_request: Ignoring request for station {request_station_id} (we are station {station_id})",
        );
        return Err(anyhow!(
            "Request for different station ID: {request_station_id} (we are {station_id})",
        ));
    }

    log::info!("respond_to_request: Received request from slave: {request:02X?}");

    // Parse and respond to request
    let mut context = storage.lock().unwrap();
    let mut response_buf = Vec::new();
    let mut frame = ModbusFrame::new(station_id, request, ModbusProto::Rtu, &mut response_buf);
    frame.parse()?;

    log::debug!(
        "respond_to_request: Parsed frame - func={:?}, reg_addr=0x{:04X?}, count={}",
        frame.func,
        frame.reg,
        frame.count
    );

    let response = match frame.func {
        rmodbus::consts::ModbusFunction::GetHoldings => {
            match build_slave_holdings_response(&mut frame, &mut context) {
                Ok(Some(resp)) => {
                    log::debug!(
                        "respond_to_request: Built holdings response ({} bytes)",
                        resp.len()
                    );
                    resp
                }
                _ => {
                    log::error!("respond_to_request: Failed to build holdings response");
                    return Err(anyhow!("Failed to build holdings response"));
                }
            }
        }
        rmodbus::consts::ModbusFunction::GetCoils => {
            match build_slave_coils_response(&mut frame, &mut context) {
                Ok(Some(resp)) => {
                    log::debug!(
                        "respond_to_request: Built coils response ({} bytes)",
                        resp.len()
                    );
                    resp
                }
                _ => {
                    log::error!("respond_to_request: Failed to build coils response");
                    return Err(anyhow!("Failed to build coils response"));
                }
            }
        }
        _ => {
            log::error!(
                "respond_to_request: Unsupported function code: {:?}",
                frame.func
            );
            return Err(anyhow!("Unsupported function code: {:?}", frame.func));
        }
    };

    drop(context);

    // Send response
    let mut port = port_arc.lock().unwrap();
    port.write_all(&response)?;
    port.flush()?;
    drop(port);

    log::info!("respond_to_request: Sent response to slave: {response:02X?}");

    // Extract values from response for JSON output
    let values = extract_values_from_response(&response)?;
    log::debug!("respond_to_request: Extracted values for output: {values:?}");

    Ok(ModbusResponse {
        station_id,
        register_address: 0, // left as 0; we parse ranges from raw request in caller
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
    changed_ranges: Arc<Mutex<Vec<(u16, u16, Instant)>>>,
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

                            // Record changed range for other thread to detect overlap
                            {
                                let len = values.len() as u16;
                                let mut cr = changed_ranges.lock().unwrap();
                                cr.push((register_address, len, Instant::now()));
                                // Keep size bounded: trim old entries
                                while cr.len() > 1000 {
                                    cr.remove(0);
                                }
                            }

                            // Wait a bit before next update to avoid overwhelming
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(err) => {
                            log::warn!("Error parsing data line: {err}");
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

                            // Record changed range for other thread to detect overlap
                            {
                                let len = values.len() as u16;
                                let mut cr = changed_ranges.lock().unwrap();
                                cr.push((register_address, len, Instant::now()));
                                while cr.len() > 1000 {
                                    cr.remove(0);
                                }
                            }

                            // Wait a bit before next update
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(err) => {
                            log::warn!("Error parsing data line: {err}");
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
