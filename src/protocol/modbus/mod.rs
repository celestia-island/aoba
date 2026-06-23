#![allow(clippy::wildcard_enum_match_arm)]
mod frame;
mod header;
mod pull_get_coils;
mod pull_get_discrete_inputs;
mod pull_get_holdings;
mod pull_get_inputs;
mod pull_set_coils;
mod pull_set_holdings;
mod slave_coils;
mod slave_discrete_inputs;
mod slave_holdings;
mod slave_inputs;

use anyhow::{anyhow, Result};
use bytes::Bytes;

use flume::{Receiver, Sender};
pub use frame::read_modbus_frame;
pub use header::parse_modbus_header;
pub use pull_get_coils::{generate_pull_get_coils_request, parse_pull_get_coils};
pub use pull_get_discrete_inputs::{
    generate_pull_get_discrete_inputs_request, parse_pull_get_discrete_inputs,
};
pub use pull_get_holdings::{generate_pull_get_holdings_request, parse_pull_get_holdings};
pub use pull_get_inputs::{generate_pull_get_inputs_request, parse_pull_get_inputs};
pub use pull_set_coils::generate_pull_set_coils_request;
pub use pull_set_holdings::{
    generate_pull_set_holding_request, generate_pull_set_holdings_bulk_request,
};
use rmodbus::{
    client::ModbusRequest,
    consts::ModbusFunction,
    server::{storage::ModbusStorageSmall, ModbusFrame},
    ModbusProto,
};
pub use slave_coils::build_slave_coils_response;
pub use slave_discrete_inputs::build_slave_discrete_inputs_response;
pub use slave_holdings::build_slave_holdings_response;
pub use slave_inputs::build_slave_inputs_response;

#[allow(clippy::too_many_lines)]
pub fn boot_modbus_slave_service(
    id: u8,
    mut context: ModbusStorageSmall,
    request_receiver: &Receiver<Bytes>,
    response_sender: &Sender<Bytes>,
) -> Result<()> {
    fn crc16_modbus(data: &[u8]) -> u16 {
        super::status::crc16_modbus(data)
    }

    // Detect duplicated payload repetitions (e.g. data body repeated twice or three times)
    // and trim to a single copy, fixing byte count & CRC. Applies to standard read functions 0x01..x04.
    fn trim_duplicate_payload(func: u8, frame: &mut Vec<u8>) -> Result<()> {
        if frame.len() < 5 {
            return Err(anyhow!(
                "Frame too short: need at least 5 bytes (id, func, byte count, data, crc)"
            ));
        }
        match func {
            0x01..=0x04 => {}
            _ => return Err(anyhow!("Unsupported function code: 0x{func:02X}")),
        }
        let original_len = frame.len();
        let (byte_count_index, data_start) = (2usize, 3usize);
        if frame.len() < data_start + 1 + 2 {
            return Err(anyhow!(
                "Frame too short: need at least one data byte plus CRC"
            ));
        }
        let reported_bc = frame[byte_count_index] as usize;

        if frame.len() < data_start + reported_bc + 2 {
            return Err(anyhow!(
                "Frame data segment too short: reported {} bytes but only {} available",
                reported_bc,
                frame.len() - data_start - 2
            ));
        }
        let data_total = frame.len() - data_start - 2;
        if data_total == reported_bc {
            return Ok(());
        }

        if reported_bc == 0 || !data_total.is_multiple_of(reported_bc) {
            return Err(anyhow!(
                "Data length {data_total} is not a multiple of reported byte count {reported_bc}"
            ));
        }
        let mult = data_total / reported_bc;
        if mult <= 1 || mult > 3 {
            return Err(anyhow!(
                "Unexpected duplicate multiplier: {mult} (expected 2 or 3)"
            ));
        }

        let first = &frame[data_start..data_start + reported_bc];
        for i in 1..mult {
            if &frame[data_start + i * reported_bc..data_start + (i + 1) * reported_bc] != first {
                return Err(anyhow!(
                    "Duplicate segments at offset {i} do not match first segment"
                ));
            }
        }

        // Trim to single copy
        frame.truncate(data_start + reported_bc); // keep header + one copy data

        // Recompute CRC
        let crc = crc16_modbus(&frame[..]);
        frame.push((crc & 0xFF) as u8);
        frame.push((crc >> 8) as u8);
        log::warn!(
            "Trimmed duplicated Modbus payload (func=0x{func:02X}, mult={mult}): old_len={original_len} new_len={}",
            frame.len()
        );
        Ok(())
    }

    // Track last response to optionally suppress exact duplicates emitted too fast
    let mut last_response: Option<Vec<u8>> = None;

    while let Ok(request) = request_receiver.recv() {
        log::info!(
            "Received Modbus request: {}",
            request
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let mut response = Vec::new();
        let mut frame = ModbusFrame::new(id, request.as_ref(), ModbusProto::Rtu, &mut response);
        frame.parse()?;

        match frame.func {
            ModbusFunction::GetCoils => {
                // Coils
                if let Ok(Some(ret)) = build_slave_coils_response(&mut frame, &mut context) {
                    log::info!(
                        "Parsed slave coils: {}",
                        ret.iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    let mut ret = ret; // make mutable for trimming
                    trim_duplicate_payload(0x01, &mut ret)?;
                    let duplicate = last_response.as_ref().is_some_and(|v| v == &ret);
                    if duplicate {
                        log::warn!("Detected immediate duplicate response (func=0x01), suppressing extra send.");
                    } else {
                        last_response = Some(ret.clone());
                        response_sender.send(Bytes::from(ret))?;
                    }
                } else {
                    log::warn!("Failed to parse slave coils");
                }
            }
            ModbusFunction::GetDiscretes => {
                // Discrete Inputs
                if let Ok(Some(ret)) =
                    build_slave_discrete_inputs_response(&mut frame, &mut context)
                {
                    log::info!(
                        "Parsed slave discrete inputs: {}",
                        ret.iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    let mut ret = ret;
                    trim_duplicate_payload(0x02, &mut ret)?;
                    let duplicate = last_response.as_ref().is_some_and(|v| v == &ret);
                    if duplicate {
                        log::warn!("Detected immediate duplicate response (func=0x02), suppressing extra send.");
                    } else {
                        last_response = Some(ret.clone());
                        response_sender.send(Bytes::from(ret))?;
                    }
                } else {
                    log::warn!("Failed to parse slave discrete inputs");
                }
            }
            ModbusFunction::GetHoldings => {
                // Holding Registers
                if let Ok(Some(ret)) = build_slave_holdings_response(&mut frame, &mut context) {
                    log::info!(
                        "Parsed slave holdings: {}",
                        ret.iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    let mut ret = ret;
                    trim_duplicate_payload(0x03, &mut ret)?;
                    let duplicate = last_response.as_ref().is_some_and(|v| v == &ret);
                    if duplicate {
                        log::warn!("Detected immediate duplicate response (func=0x03), suppressing extra send.");
                    } else {
                        last_response = Some(ret.clone());
                        response_sender.send(Bytes::from(ret))?;
                    }
                } else {
                    log::warn!("Failed to parse slave holdings");
                }
            }
            ModbusFunction::GetInputs => {
                // Input Registers
                if let Ok(Some(ret)) = build_slave_inputs_response(&mut frame, &mut context) {
                    log::info!(
                        "Parsed slave input registers: {}",
                        ret.iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    let mut ret = ret;
                    trim_duplicate_payload(0x04, &mut ret)?;
                    let duplicate = last_response.as_ref().is_some_and(|v| v == &ret);
                    if duplicate {
                        log::warn!("Detected immediate duplicate response (func=0x04), suppressing extra send.");
                    } else {
                        last_response = Some(ret.clone());
                        response_sender.send(Bytes::from(ret))?;
                    }
                } else {
                    log::warn!("Failed to parse slave input registers");
                }
            }
            _ => {
                log::warn!("Unsupported function code: {:?}", frame.func);
            }
        }
    }

    Ok(())
}

/// Check if a port name represents a virtual port (IPC/HTTP) rather than a physical serial port.
/// This is a convenience wrapper around `PortType::detect().is_virtual()`.
#[must_use]
pub fn is_virtual_port(port_name: &str) -> bool {
    use crate::protocol::status::types::port::PortType;
    PortType::detect(port_name).is_virtual()
}

/// Validate and parse a Modbus RTU pull set response.
pub fn parse_pull_set_response(request: &mut ModbusRequest, response: &[u8]) -> Result<()> {
    request.parse_ok(response)?;

    Ok(())
}
