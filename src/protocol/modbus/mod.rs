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

use anyhow::Result;
use bytes::Bytes;
use chrono::Duration;
use flume::{Receiver, Sender};

use rmodbus::{
    client::ModbusRequest,
    server::{storage::ModbusStorageSmall, ModbusFrame},
    ModbusProto,
};

pub use frame::read_modbus_frame;
pub use header::parse_modbus_header;
pub use pull_get_coils::{generate_pull_get_coils_request, parse_pull_get_coils};
pub use pull_get_discrete_inputs::{
    generate_pull_get_discrete_inputs_request, parse_pull_get_discrete_inputs,
};
pub use pull_get_holdings::{generate_pull_get_holdings_request, parse_pull_get_holdings};
pub use pull_get_inputs::{generate_pull_get_inputs_request, parse_pull_get_inputs};
pub use pull_set_coils::generate_pull_set_coils_request;
pub use pull_set_holdings::generate_pull_set_holding_request;
pub use slave_coils::build_slave_coils_response;
pub use slave_discrete_inputs::build_slave_discrete_inputs_response;
pub use slave_holdings::build_slave_holdings_response;
pub use slave_inputs::build_slave_inputs_response;

pub fn boot_modbus_pull_service(id: u8, request_sender: Sender<Bytes>) -> Result<()> {
    let request_tx = request_sender.to_owned();

    let mut last_sent_timestamp = chrono::Utc::now();

    #[derive(Debug, Clone, Copy)]
    enum PollTask {
        GetCoils,
        GetHoldings,
    }

    impl PollTask {
        fn next(&self) -> Self {
            match self {
                PollTask::GetCoils => PollTask::GetHoldings,
                PollTask::GetHoldings => PollTask::GetCoils,
            }
        }

        fn wait_next_duration(&self) -> Duration {
            match self {
                PollTask::GetCoils => Duration::seconds(2),
                PollTask::GetHoldings => Duration::seconds(1),
            }
        }
    }

    let mut current_task = PollTask::GetCoils;

    loop {
        let now = chrono::Utc::now();
        if request_tx.is_empty() && now - last_sent_timestamp > current_task.wait_next_duration() {
            log::info!("Sending Modbus pull request for {current_task:?}");
            // Periodically send data pull requests
            let mut request = ModbusRequest::new(id, ModbusProto::Rtu);
            let mut raw = Vec::new();
            match current_task {
                PollTask::GetCoils => request.generate_get_coils(1, 11, &mut raw)?,
                PollTask::GetHoldings => request.generate_get_holdings(16, 33, &mut raw)?,
            }
            request_sender.send(Bytes::from(raw))?;

            last_sent_timestamp = chrono::Utc::now();
            current_task = current_task.next();
        }
    }
}

pub fn boot_modbus_slave_service(
    id: u8,
    mut context: ModbusStorageSmall,
    request_receiver: Receiver<Bytes>,
    response_sender: Sender<Bytes>,
) -> Result<()> {
    // Track last response to optionally suppress exact duplicates emitted too fast
    let mut last_response: Option<Vec<u8>> = None;

    fn crc16_modbus(data: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;
        for &b in data {
            crc ^= b as u16;
            for _ in 0..8 {
                if crc & 0x0001 != 0 {
                    crc >>= 1;
                    crc ^= 0xA001;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc
    }

    // Detect duplicated payload repetitions (e.g. data body repeated twice or three times)
    // and trim to a single copy, fixing byte count & CRC. Applies to standard read functions 0x01..0x04.
    fn trim_duplicate_payload(func: u8, frame: &mut Vec<u8>) -> bool {
        if frame.len() < 5 {
            // minimal: sid func bc data crc1 crc2
            return false;
        }
        match func {
            0x01..=0x04 => {}
            _ => return false,
        }
        let original = frame.clone();
        let (byte_count_idx, data_start) = (2usize, 3usize);
        if frame.len() < data_start + 1 + 2 {
            // need at least one data byte + crc
            return false;
        }
        let reported_bc = frame[byte_count_idx] as usize;
        // data segment excluding crc
        if frame.len() < data_start + reported_bc + 2 {
            return false;
        }
        let data_total = frame.len() - data_start - 2; // actual data bytes present
        if data_total == reported_bc {
            return false;
        } // already consistent
          // Check if data_total is an integer multiple of reported_bc (2x or 3x)
        if reported_bc == 0 || data_total % reported_bc != 0 {
            return false;
        }
        let mult = data_total / reported_bc;
        if mult <= 1 || mult > 3 {
            return false;
        }
        // Verify repetition segments identical
        let first = &frame[data_start..data_start + reported_bc];
        for i in 1..mult {
            if &frame[data_start + i * reported_bc..data_start + (i + 1) * reported_bc] != first {
                return false;
            }
        }
        // Trim to single copy
        frame.truncate(data_start + reported_bc); // keep header + one copy data
                                                  // Recompute CRC
        let crc = crc16_modbus(&frame[..]);
        frame.push((crc & 0xFF) as u8);
        frame.push((crc >> 8) as u8);
        log::warn!(
            "Trimmed duplicated Modbus payload (func=0x{:02X}, mult={}): old_len={} new_len={}",
            func,
            mult,
            original.len(),
            frame.len()
        );
        true
    }

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
            0x01 => {
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
                    let _ = trim_duplicate_payload(0x01, &mut ret);
                    let duplicate = last_response.as_ref().map(|v| v == &ret).unwrap_or(false);
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
            0x02 => {
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
                    let _ = trim_duplicate_payload(0x02, &mut ret);
                    let duplicate = last_response.as_ref().map(|v| v == &ret).unwrap_or(false);
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
            0x03 => {
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
                    let _ = trim_duplicate_payload(0x03, &mut ret);
                    let duplicate = last_response.as_ref().map(|v| v == &ret).unwrap_or(false);
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
            0x04 => {
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
                    let _ = trim_duplicate_payload(0x04, &mut ret);
                    let duplicate = last_response.as_ref().map(|v| v == &ret).unwrap_or(false);
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
                log::warn!("Unsupported function code: {}", frame.func);
            }
        }
    }

    Ok(())
}

/// Validate and parse a Modbus RTU pull set response.
pub fn parse_pull_set_response(request: &mut ModbusRequest, response: Vec<u8>) -> Result<()> {
    request.parse_ok(&response)?;

    Ok(())
}
