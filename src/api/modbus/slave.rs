use anyhow::{anyhow, Result};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{extract_values_from_storage, ModbusHook, ModbusPortConfig, ModbusResponse};
use crate::api::utils::open_serial_port;

pub async fn run_slave_loop(
    config: ModbusPortConfig,
    hooks: Option<Arc<dyn ModbusHook>>,
) -> Result<()> {
    log::info!("Starting slave loop for {}", config.port_name);

    let port_handle = open_serial_port(
        &config.port_name,
        config.baud_rate,
        Duration::from_millis(config.timeout_ms),
    )?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    loop {
        if let Some(h) = &hooks {
            if let Err(e) = h.on_before_request(&config.port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        match listen_for_one_request(
            port_arc.clone(),
            config.station_id,
            config.register_address,
            config.register_length,
            config.register_mode,
            storage.clone(),
        ) {
            Ok(response) => {
                if let Some(h) = &hooks {
                    if let Err(e) = h.on_after_response(&config.port_name, &response) {
                        log::warn!("Hook on_after_response failed: {}", e);
                    }
                }
            }
            Err(err) => {
                log::warn!("Error processing request on {}: {}", config.port_name, err);
                if let Some(h) = &hooks {
                    h.on_error(&config.port_name, &err);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Listen for one Modbus request and respond (Slave/Server logic)
pub fn listen_for_one_request(
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
    log::debug!("Received request: {request:02X?}");

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
        log::debug!("Sent response: {resp:02X?}");
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
