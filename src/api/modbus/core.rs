/// Core Modbus communication logic - Pure functions without channel dependencies
///
/// This module provides the fundamental Modbus protocol operations:
/// - Slave: listen for requests and generate responses
/// - Master: send requests and parse responses
///
/// These functions are pure and don't depend on specific communication channels.
use anyhow::{anyhow, Result};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use crate::protocol::status::types::modbus::{ModbusResponse, RegisterMode, ResponseRegisterMode};

/// Listen for one Modbus request and respond (Slave/Server logic)
///
/// This is a pure function that handles the Modbus protocol without any channel dependencies.
/// It reads from the port, processes the request, sends a response, and returns the data.
pub fn slave_process_one_request(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    _reg_mode: RegisterMode,
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
) -> Result<ModbusResponse> {
    use rmodbus::{server::ModbusFrame, ModbusProto};

    // Read request from port with retry for complete frame
    // Modbus RTU frames are typically 8+ bytes, but may arrive in fragments
    let mut buffer = vec![0u8; 256];
    let mut total_bytes = 0;
    let mut port = port_arc.lock().unwrap();

    // First read - get initial data
    let bytes_read = port.read(&mut buffer[total_bytes..])?;
    if bytes_read == 0 {
        drop(port);
        return Err(anyhow!("No data received"));
    }
    total_bytes += bytes_read;

    // Wait a bit for remaining data (Modbus RTU inter-frame delay)
    // At 57600 baud, 8 bytes takes ~1.4ms, give it up to 10ms
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Try to read any remaining bytes
    if let Ok(additional) = port.read(&mut buffer[total_bytes..]) {
        total_bytes += additional;
    }

    drop(port);

    let request = &buffer[..total_bytes];
    log::debug!("Received request ({} bytes): {request:02X?}", total_bytes);

    // Parse and respond to request
    let mut response = Vec::new();
    let mut frame = ModbusFrame::new(station_id, request, ModbusProto::Rtu, &mut response);
    frame.parse()?;

    // Determine the actual register mode from the received function code
    use rmodbus::consts::ModbusFunction;
    let actual_mode = match frame.func {
        ModbusFunction::GetHoldings => RegisterMode::Holding,
        ModbusFunction::GetInputs => RegisterMode::Input,
        ModbusFunction::GetCoils => RegisterMode::Coils,
        ModbusFunction::GetDiscretes => RegisterMode::DiscreteInputs,
        _ => {
            return Err(anyhow!(
                "Unsupported function code: 0x{:02X} ({:?})",
                frame.func as u8,
                frame.func
            ));
        }
    };

    log::debug!(
        "Processing {:?} request (func code 0x{:02X})",
        actual_mode,
        frame.func as u8
    );

    // Generate response based on the actual function code in the request
    let response_bytes = match actual_mode {
        RegisterMode::Holding => crate::protocol::modbus::build_slave_holdings_response(
            &mut frame,
            &mut storage.lock().unwrap(),
        )?,
        RegisterMode::Input => crate::protocol::modbus::build_slave_inputs_response(
            &mut frame,
            &mut storage.lock().unwrap(),
        )?,
        RegisterMode::Coils => crate::protocol::modbus::build_slave_coils_response(
            &mut frame,
            &mut storage.lock().unwrap(),
        )?,
        RegisterMode::DiscreteInputs => {
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

    // Extract values from storage for response (use actual mode, not configured mode)
    let values =
        extract_values_from_storage(&storage, register_address, register_length, actual_mode)?;

    Ok(ModbusResponse {
        station_id,
        register_address,
        register_mode: ResponseRegisterMode::from_register_mode(actual_mode),
        values,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Execute a single master poll transaction (Master/Client logic)
///
/// This is a pure function that sends a request and parses the response.
/// No channel dependencies - just returns the parsed data.
pub fn master_poll_once(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: RegisterMode,
) -> Result<ModbusResponse> {
    let request_bytes = match reg_mode {
        RegisterMode::Holding => crate::protocol::modbus::generate_pull_get_holdings_request(
            station_id,
            register_address,
            register_length,
        )?,
        RegisterMode::Input => crate::protocol::modbus::generate_pull_get_inputs_request(
            station_id,
            register_address,
            register_length,
        )?,
        RegisterMode::Coils => crate::protocol::modbus::generate_pull_get_coils_request(
            station_id,
            register_address,
            register_length,
        )?,
        RegisterMode::DiscreteInputs => {
            crate::protocol::modbus::generate_pull_get_discrete_inputs_request(
                station_id,
                register_address,
                register_length,
            )?
        }
    };

    let request_frame = request_bytes.1;

    log::debug!("Sending request to slave: {request_frame:02X?}");
    {
        let mut port = port_arc.lock().unwrap();
        port.write_all(&request_frame)?;
        port.flush()?;
    }

    // Read response with retry for complete frame (same as slave logic)
    // Modbus responses can be large (e.g., 71 bytes for 33 registers)
    let mut buffer = vec![0u8; 256];
    let mut total_bytes = 0;

    // First read - get initial data
    {
        let mut port = port_arc.lock().unwrap();
        let bytes_read = port.read(&mut buffer[total_bytes..])?;
        if bytes_read == 0 {
            return Err(anyhow!("No response received"));
        }
        total_bytes += bytes_read;
    }

    // Wait for remaining data (Modbus RTU inter-frame delay)
    // At 57600 baud, 71 bytes takes ~12ms, give it up to 20ms
    std::thread::sleep(std::time::Duration::from_millis(20));

    // Try to read any remaining bytes
    {
        let mut port = port_arc.lock().unwrap();
        if let Ok(additional) = port.read(&mut buffer[total_bytes..]) {
            total_bytes += additional;
        }
    }

    let response = &buffer[..total_bytes];
    log::debug!(
        "Received response from slave ({} bytes): {response:02X?}",
        total_bytes
    );

    // Parse response values
    let values = parse_response_values(response, register_length, reg_mode)?;
    log::debug!("Parsed {} values from response", values.len());

    Ok(ModbusResponse {
        station_id,
        register_address,
        register_mode: ResponseRegisterMode::from_register_mode(reg_mode),
        values,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Parse response values from raw bytes
fn parse_response_values(
    response: &[u8],
    register_length: u16,
    reg_mode: RegisterMode,
) -> Result<Vec<u16>> {
    match reg_mode {
        RegisterMode::Holding | RegisterMode::Input => {
            if response.len() < 5 {
                return Err(anyhow!(
                    "Response too short (len={}, need >=5)",
                    response.len()
                ));
            }
            let byte_count = response[2] as usize;
            log::debug!(
                "Parsing Holding/Input: byte_count={}, response.len()={}",
                byte_count,
                response.len()
            );
            let mut values = Vec::new();
            for i in 0..(byte_count / 2) {
                let offset = 3 + i * 2;
                if offset + 1 < response.len() {
                    let value = u16::from_be_bytes([response[offset], response[offset + 1]]);
                    values.push(value);
                } else {
                    log::warn!(
                        "Incomplete register at offset {}: response too short",
                        offset
                    );
                }
            }
            log::debug!("Parsed {} register values", values.len());
            Ok(values)
        }
        RegisterMode::Coils | RegisterMode::DiscreteInputs => {
            if response.len() < 5 {
                return Err(anyhow!("Response too short"));
            }
            let byte_count = response[2] as usize;
            let mut values = Vec::new();
            for byte_idx in 0..byte_count {
                if 3 + byte_idx < response.len() {
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
                }
            }
            values.truncate(register_length as usize);
            Ok(values)
        }
    }
}

fn extract_values_from_storage(
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    start_addr: u16,
    length: u16,
    reg_mode: RegisterMode,
) -> Result<Vec<u16>> {
    use rmodbus::server::context::ModbusContext;

    let storage = storage.lock().unwrap();
    let mut values = Vec::new();

    for i in 0..length {
        let addr = start_addr + i;
        let value = match reg_mode {
            RegisterMode::Holding => storage.get_holding(addr)?,
            RegisterMode::Input => storage.get_input(addr)?,
            RegisterMode::Coils => {
                if storage.get_coil(addr)? {
                    1
                } else {
                    0
                }
            }
            RegisterMode::DiscreteInputs => {
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
