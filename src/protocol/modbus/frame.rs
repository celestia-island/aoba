use anyhow::{anyhow, Result};
use bytes::Bytes;
use chrono::Duration;
use std::sync::{Arc, Mutex};

use serialport::SerialPort;

use super::parse_modbus_header;

/// Read a Modbus RTU frame from the provided serial port wrapper.
/// Returns Ok(Some(Bytes)) when a full frame is read, Ok(None) for timeout / no data,
/// or Err for unexpected I/O / locking errors.
pub fn read_modbus_frame(usbtty: Arc<Mutex<Box<dyn SerialPort + Send>>>) -> Result<Option<Bytes>> {
    // Incremental read helper: read until we have 'need' bytes or the port times out.
    fn read_until(
        port: &mut dyn SerialPort,
        target: &mut Vec<u8>,
        need: usize,
    ) -> std::io::Result<()> {
        while target.len() < need {
            let mut buf = [0u8; 64];
            let want = (need - target.len()).min(buf.len());
            match port.read(&mut buf[..want]) {
                Ok(0) => break, // no more for now (respect timeout at higher layer)
                Ok(n) => target.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    let start = chrono::Utc::now();
    let mut collected: Vec<u8> = Vec::with_capacity(64);
    // Read header (6 bytes) with overall 2 s deadline
    while collected.len() < 6 {
        {
            let mut guard = usbtty
                .lock()
                .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?;
            read_until(&mut **guard, &mut collected, 6)?;
        }
        if collected.len() >= 6 {
            break;
        }
        if chrono::Utc::now() - start > Duration::seconds(2) {
            return Ok(None);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if collected.len() < 6 {
        return Ok(None);
    }
    let mut header = [0u8; 6];
    header.copy_from_slice(&collected[..6]);
    let guessed_len = match parse_modbus_header(header) {
        Ok(v) => v,
        Err(_) => {
            log::warn!("Failed to parse Modbus header");
            return Ok(None);
        }
    };
    // Modbus RTU maximum 256 bytes
    if guessed_len < 4 || guessed_len > 256 {
        log::warn!("Guessed invalid frame length: {guessed_len}");
        return Ok(None);
    }
    // Read the remaining bytes with a 3 s additional deadline
    let body_deadline = chrono::Utc::now() + Duration::seconds(3);
    while collected.len() < guessed_len {
        // guessed_len includes header
        {
            let mut guard = usbtty
                .lock()
                .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?;
            read_until(&mut **guard, &mut collected, guessed_len)?;
        }
        if collected.len() >= guessed_len {
            break;
        }
        if chrono::Utc::now() > body_deadline {
            log::warn!(
                "Frame read timeout (expected {guessed_len}, got {})",
                collected.len()
            );
            return Ok(None);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    if collected.len() != guessed_len {
        return Ok(None);
    }

    // CRC check (little endian stored: low then high)
    if guessed_len >= 4 {
        let data_no_crc_len = guessed_len - 2;
        let calc = crc16_modbus(&collected[..data_no_crc_len]);
        let frame_crc =
            (collected[data_no_crc_len] as u16) | ((collected[data_no_crc_len + 1] as u16) << 8);
        if calc != frame_crc {
            log::warn!("CRC mismatch: calc=0x{calc:04X} frame=0x{frame_crc:04X}");
            return Ok(None);
        }
    }

    // Flush serial output buffers (optional safety)
    usbtty
        .lock()
        .map_err(|e| anyhow!("Failed to lock USBTTY port: {}", e))?
        .flush()?;

    log::debug!("Received Modbus frame ({} bytes)", collected.len());
    Ok(Some(Bytes::from(collected)))
}

// Local CRC16 (Modbus) implementation
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
