use anyhow::{anyhow, Result};
use clap::ArgMatches;
use rmodbus::server::context::ModbusContext;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Response structure for modbus operations
#[derive(Serialize, Clone)]
pub struct ModbusResponse {
    pub station_id: u8,
    pub register_address: u16,
    pub register_mode: String,
    pub values: Vec<u16>,
    pub timestamp: String,
}

/// Data source for master mode
pub enum DataSource {
    File(String),
    Pipe(String),
}

impl std::str::FromStr for DataSource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("file:") {
            Ok(DataSource::File(path.to_string()))
        } else if let Some(name) = s.strip_prefix("pipe:") {
            Ok(DataSource::Pipe(name.to_string()))
        } else {
            Err(anyhow!(
                "Invalid data source format. Use file:<path> or pipe:<name>"
            ))
        }
    }
}

/// Parse register mode from string
fn parse_register_mode(mode: &str) -> Result<crate::protocol::status::types::modbus::RegisterMode> {
    use crate::protocol::status::types::modbus::RegisterMode;
    match mode.to_lowercase().as_str() {
        "holding" => Ok(RegisterMode::Holding),
        "input" => Ok(RegisterMode::Input),
        "coils" => Ok(RegisterMode::Coils),
        "discrete" => Ok(RegisterMode::DiscreteInputs),
        _ => Err(anyhow!("Invalid register mode: {}", mode)),
    }
}

/// Handle slave listen (temporary: output once and exit)
pub fn handle_slave_listen(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting slave listen on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Open serial port
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_secs(5))
        .open()
        .map_err(|e| anyhow!("Failed to open port {}: {}", port, e))?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    // Wait for one request and respond
    let response = listen_for_one_request(
        port_arc,
        station_id,
        register_address,
        register_length,
        reg_mode,
        storage,
    )?;

    // Output JSON
    let json = serde_json::to_string(&response)?;
    println!("{json}");

    Ok(())
}

/// Handle slave listen persist (continuous JSONL output)
pub fn handle_slave_listen_persist(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();

    let reg_mode = parse_register_mode(register_mode)?;

    log::info!(
        "Starting persistent slave listen on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Open serial port
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| anyhow!("Failed to open port {}: {}", port, e))?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    // Continuously listen and output JSONL
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
                let json = serde_json::to_string(&response)?;
                println!("{json}");
            }
            Err(e) => {
                log::warn!("Error processing request: {e}");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

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
    let data_source = DataSource::from_str(data_source_str)?;

    log::info!(
        "Starting master provide on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Open serial port
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_secs(5))
        .open()
        .map_err(|e| anyhow!("Failed to open port {}: {}", port, e))?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Read one line of data and provide it
    let values = read_one_data_update(&data_source)?;

    let response = provide_data_once(
        port_arc,
        station_id,
        register_address,
        register_length,
        reg_mode,
        values,
    )?;

    // Output JSON
    let json = serde_json::to_string(&response)?;
    println!("{json}");

    Ok(())
}

/// Handle master provide persist (continuous JSONL output)
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
    let data_source = DataSource::from_str(data_source_str)?;

    log::info!(
        "Starting persistent master provide on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Open serial port
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| anyhow!("Failed to open port {}: {}", port, e))?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Continuously read data updates and provide them
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
                        match provide_data_once(
                            port_arc.clone(),
                            station_id,
                            register_address,
                            register_length,
                            reg_mode,
                            values,
                        ) {
                            Ok(response) => {
                                let json = serde_json::to_string(&response)?;
                                println!("{json}");
                            }
                            Err(e) => {
                                log::warn!("Error providing data: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Error parsing data line: {e}");
                    }
                }
            }
        }
        DataSource::Pipe(_name) => {
            // Named pipe support requires platform-specific implementation
            return Err(anyhow!("Named pipe support is not yet implemented"));
        }
    }

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

/// Provide data once as master
fn provide_data_once(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    _register_length: u16,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    values: Vec<u16>,
) -> Result<ModbusResponse> {
    use rmodbus::client::ModbusRequest;
    use rmodbus::ModbusProto;

    // Create write request
    let mut request = ModbusRequest::new(station_id, ModbusProto::Rtu);
    let mut raw = Vec::new();

    match reg_mode {
        crate::protocol::status::types::modbus::RegisterMode::Holding => {
            // Write holding registers
            request.generate_set_holdings_bulk(register_address, &values, &mut raw)?;
        }
        crate::protocol::status::types::modbus::RegisterMode::Coils => {
            // Write coils
            let coils: Vec<bool> = values.iter().map(|&v| v != 0).collect();
            request.generate_set_coils_bulk(register_address, &coils, &mut raw)?;
        }
        _ => {
            return Err(anyhow!(
                "Master can only write to holding registers or coils"
            ));
        }
    }

    // Send request
    let mut port = port_arc.lock().unwrap();
    port.write_all(&raw)?;
    port.flush()?;
    log::info!("Sent write request: {raw:02X?}");

    // Read response
    let mut buffer = vec![0u8; 256];
    let bytes_read = port.read(&mut buffer)?;
    drop(port);

    if bytes_read > 0 {
        log::info!(
            "Received response: {resp_bytes:02X?}",
            resp_bytes = &buffer[..bytes_read]
        );
    }

    Ok(ModbusResponse {
        station_id,
        register_address,
        register_mode: format!("{reg_mode:?}"),
        values,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
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
        DataSource::Pipe(_name) => {
            // Named pipe support requires platform-specific implementation
            // For now, return an error
            Err(anyhow!("Named pipe support is not yet implemented"))
        }
    }
}

/// Parse a data line in JSON format
fn parse_data_line(line: &str) -> Result<Vec<u16>> {
    let line = line.trim();
    if line.is_empty() {
        return Err(anyhow!("Empty line"));
    }

    // Try to parse as JSON object with "values" field
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
        if let Some(values) = json.get("values") {
            if let Some(arr) = values.as_array() {
                let mut result = Vec::new();
                for val in arr {
                    if let Some(num) = val.as_u64() {
                        result.push(num as u16);
                    }
                }
                return Ok(result);
            }
        }
    }

    Err(anyhow!("Invalid data format"))
}

/// Extract values from modbus storage
fn extract_values_from_storage(
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    start_addr: u16,
    length: u16,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
) -> Result<Vec<u16>> {
    let storage = storage.lock().unwrap();
    let mut values = Vec::new();

    for i in 0..length {
        let addr = start_addr + i;
        let value = match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                storage.get_holding(addr)?
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => {
                storage.get_input(addr)?
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                if storage.get_coil(addr)? {
                    1
                } else {
                    0
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                if storage.get_discrete(addr)? {
                    1
                } else {
                    0
                }
            }
        };
        values.push(value);
    }

    Ok(values)
}
