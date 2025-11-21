use anyhow::{anyhow, Result};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{ModbusHook, ModbusPortConfig, ModbusResponse};
use crate::api::utils::open_serial_port;

pub async fn run_master_loop(
    config: ModbusPortConfig,
    hooks: Option<Arc<dyn ModbusHook>>,
) -> Result<()> {
    log::info!("Starting master loop for {}", config.port_name);

    let port_handle = open_serial_port(
        &config.port_name,
        config.baud_rate,
        Duration::from_millis(config.timeout_ms),
    )?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    loop {
        if let Some(h) = &hooks {
            if let Err(e) = h.on_before_request(&config.port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        match run_master_poll_transaction(
            port_arc.clone(),
            config.station_id,
            config.register_address,
            config.register_length,
            config.register_mode,
        ) {
            Ok(response) => {
                if let Some(h) = &hooks {
                    if let Err(e) = h.on_after_response(&config.port_name, &response) {
                        log::warn!("Hook on_after_response failed: {}", e);
                    }
                }
            }
            Err(err) => {
                log::warn!("Error polling on {}: {}", config.port_name, err);
                if let Some(h) = &hooks {
                    h.on_error(&config.port_name, &err);
                }
            }
        }

        // Poll interval
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Execute a single master poll transaction (Master/Client logic)
pub fn run_master_poll_transaction(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
) -> Result<ModbusResponse> {
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

    let request_frame = request_bytes.1;

    log::debug!("Sending request to slave: {request_frame:02X?}");
    {
        let mut port = port_arc.lock().unwrap();
        port.write_all(&request_frame)?;
        port.flush()?;
    }

    let mut buffer = vec![0u8; 256];
    let bytes_read = {
        let mut port = port_arc.lock().unwrap();
        port.read(&mut buffer)?
    };

    if bytes_read == 0 {
        return Err(anyhow!("No response received"));
    }

    let response = &buffer[..bytes_read];
    log::debug!("Received response from slave: {response:02X?}");

    // Parse response values
    let values = match reg_mode {
        crate::protocol::status::types::modbus::RegisterMode::Holding
        | crate::protocol::status::types::modbus::RegisterMode::Input => {
            if bytes_read < 5 {
                return Err(anyhow!("Response too short"));
            }
            let byte_count = response[2] as usize;
            let mut values = Vec::new();
            for i in 0..(byte_count / 2) {
                let offset = 3 + i * 2;
                if offset + 1 < response.len() {
                    let value = u16::from_be_bytes([response[offset], response[offset + 1]]);
                    values.push(value);
                }
            }
            values
        }
        crate::protocol::status::types::modbus::RegisterMode::Coils
        | crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
            if bytes_read < 5 {
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
}
