use anyhow::{anyhow, Result};
use bytes::Bytes;
use chrono::Duration;
use std::sync::{Arc, Mutex};

use super::parse_modbus_header;

pub fn read_modbus_frame(
    usbtty: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    slave_ids_for_extra_crc_flag: &[u8],
) -> Result<Option<Bytes>> {
    let mut header_buf = [0u8; 6];
    if usbtty
        .lock()
        .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
        .read_exact(&mut header_buf)
        .is_err()
    {
        return Ok(None);
    }

    let len = if let Ok(len) = parse_modbus_header(header_buf) {
        len
    } else {
        log::warn!("Failed to parse Modbus header");
        usbtty
            .lock()
            .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
            .clear_break()?;
        return Ok(None);
    };
    let body_len = len.min(256).max(6);
    let mut body_buf = vec![
        0u8;
        body_len - 6
            + if slave_ids_for_extra_crc_flag.contains(&header_buf[0]) {
                2
            } else {
                0
            }
    ];
    let body_start = chrono::Utc::now();
    loop {
        if usbtty
            .lock()
            .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
            .read_exact(&mut body_buf)
            .is_ok()
        {
            break;
        }
        if chrono::Utc::now() - body_start > Duration::seconds(3) {
            log::warn!("Body read timeout");
            break;
        }
    }

    let mut full_buf = header_buf.to_vec();
    full_buf.extend_from_slice(&body_buf);

    usbtty
        .lock()
        .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
        .flush()?;
    log::info!(
        "Received Modbus request: {}",
        full_buf
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" "),
    );

    Ok(Some(Bytes::from(full_buf)))
}
