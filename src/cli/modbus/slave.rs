use anyhow::{anyhow, Result};
use clap::ArgMatches;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{extract_values_from_storage, parse_register_mode, ModbusResponse};

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
