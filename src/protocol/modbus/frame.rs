use anyhow::{anyhow, Result};
use bytes::Bytes;
use chrono::Duration;
use std::sync::{Arc, Mutex};

use serialport::SerialPort;

use super::parse_modbus_header;

/// Read a Modbus RTU frame from the provided serial port wrapper.
/// Returns Ok(Some(Bytes)) when a full frame was read, Ok(None) for timeout/no-data,
/// or Err for unexpected I/O/locking errors.
pub fn read_modbus_frame(usbtty: Arc<Mutex<Box<dyn SerialPort + Send>>>) -> Result<Option<Bytes>> {
    // Read fixed-length header first (6 bytes used by rmodbus guesser)
    let mut header_buf: [u8; 6] = [0u8; 6];

    // Try to read the header with a short wait loop to avoid blocking forever.
    let header_start = chrono::Utc::now();
    loop {
        let read_res = usbtty
            .lock()
            .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
            .read_exact(&mut header_buf);

        if read_res.is_ok() {
            break;
        }

        // bail out if we've been waiting too long
        if chrono::Utc::now() - header_start > Duration::seconds(2) {
            return Ok(None);
        }
    }

    // Determine expected total length using rmodbus helper (via parse_modbus_header)
    let len = if let Ok(len) = parse_modbus_header(header_buf) {
        len
    } else {
        log::warn!("Failed to parse Modbus header");
        // Try to clear any break condition on the port, then return None to skip this frame.
        let _ = usbtty
            .lock()
            .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
            .clear_break();
        return Ok(None);
    };

    // Clamp body length to reasonable bounds and allocate
    let body_len = len.min(256).max(6);
    let mut body_buf = vec![0u8; body_len - 6];

    // Read remaining body with a timeout
    let body_start = chrono::Utc::now();
    loop {
        let read_res = usbtty
            .lock()
            .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
            .read_exact(&mut body_buf);

        if read_res.is_ok() {
            break;
        }

        if chrono::Utc::now() - body_start > Duration::seconds(3) {
            log::warn!("Body read timeout");
            break;
        }
    }

    let mut full_buf = header_buf.to_vec();
    full_buf.extend_from_slice(&body_buf);

    // Flush any buffered writes and log the received frame
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
