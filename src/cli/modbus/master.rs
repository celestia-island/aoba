use anyhow::{anyhow, Result};
use std::{
    io::{BufRead, BufReader},
    sync::{Arc, Mutex},
    time::Duration,
};

use clap::ArgMatches;

use super::{parse_data_line, parse_register_mode, DataSource, ModbusResponse};
use crate::cli::cleanup;

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

    // Read one line of data and provide it
    let values = read_one_data_update(&data_source)?;

    let response = {
        // Open serial port in a scope to ensure it's closed before returning
        let port_handle = serialport::new(port, baud_rate)
            .timeout(Duration::from_secs(5))
            .open()
            .map_err(|e| anyhow!("Failed to open port {}: {}", port, e))?;

        let port_arc = Arc::new(Mutex::new(port_handle));

        let response = provide_data_once(
            port_arc.clone(),
            station_id,
            register_address,
            register_length,
            reg_mode,
            values,
        )?;

        // Explicitly drop port_arc to close the port
        drop(port_arc);

        // Give the OS time to fully release the port
        std::thread::sleep(Duration::from_millis(100));

        response
    };

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
    let data_source = data_source_str.parse::<DataSource>()?;

    log::info!(
        "Starting persistent master provide on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Open serial port
    let port_handle = serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| anyhow!("Failed to open port {}: {}", port, e))?;

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

    // Continuously read data updates and provide them (loop forever for persistent service)
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
                            match provide_data_once(
                                port_arc.clone(),
                                station_id,
                                register_address,
                                register_length,
                                reg_mode,
                                values.clone(),
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
                
                // After reading all lines, loop back to start of file
                log::debug!("Reached end of data file, looping back to start");
            }
            DataSource::Pipe(_name) => {
                // Named pipe support requires platform-specific implementation
                return Err(anyhow!("Named pipe support is not yet implemented"));
            }
        }
    }
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
