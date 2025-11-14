use anyhow::Result;
use bytes::Bytes;
use chrono::Duration;
use std::sync::Arc;
use tokio::sync::Mutex;

use serialport::SerialPort;

use super::parse_modbus_header;
use crate::utils::sleep::sleep_1s;

/// Read a Modbus RTU frame from the provided serial port wrapper.
/// Returns Ok(Some(Bytes)) when a full frame is read, Ok(None) for timeout / no data,
/// or Err for unexpected I / O / locking errors.
pub async fn read_modbus_frame(
    usbtty: Arc<Mutex<Box<dyn SerialPort + Send>>>,
) -> Result<Option<Bytes>> {
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
                Ok(n) => {
                    // Append newly read bytes and emit a debug trace describing the chunk
                    target.extend_from_slice(&buf[..n]);
                    if log::log_enabled!(log::Level::Debug) {
                        let chunk_hex = buf[..n]
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let so_far = target
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        log::debug!("serial read chunk={chunk_hex} collected={so_far}");
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::TimedOut => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    let start = chrono::Utc::now();
    let mut collected: Vec<u8> = Vec::with_capacity(256);

    // Acquire serial lock once for the duration of this read operation. This avoids
    // interleaved lock/unlock cycles which can split incoming data across reads
    // in a way that appears to reorder bytes from the caller's perspective.
    let mut guard = usbtty.lock().await;

    // Step 1: read minimal 2 bytes (slave id + function)
    while collected.len() < 2 {
        read_until(&mut **guard, &mut collected, 2)?;
        if collected.len() >= 2 {
            break;
        }
        if chrono::Utc::now() - start > Duration::seconds(2) {
            return Ok(None);
        }
        // yield briefly while keeping the lock (running in dedicated thread)
        // Use async sleep instead of blocking sleep
        sleep_1s().await;
    }

    if collected.len() < 2 {
        if collected.is_empty() {
            return Ok(None);
        } else {
            return Ok(Some(Bytes::from(collected)));
        }
    }

    // Helper: attempt to determine full frame length from currently collected bytes.
    // Returns Some(length) when determined, or None if undecidable yet.
    let determine_length = |col: &Vec<u8>| -> Option<usize> {
        if col.len() >= 6 {
            let mut header = [0u8; 6];
            header.copy_from_slice(&col[..6]);
            if let Ok(v) = parse_modbus_header(header) {
                return Some(v);
            } else {
                // parse_modbus_header failed; fall through to heuristic
            }
        }
        let func = col.get(1).copied().unwrap_or(0);
        // Exception response (func with MSB set) typically: id(1) + func(1) + excode(1) + crc(2) => 5
        if (func & 0x80) != 0 {
            return Some(5);
        }
        match func {
            0x01..=0x06 => {
                // These are fixed-size requests: id + func + addr(2) + qty/val(2) + crc(2) = 8
                Some(8)
            }
            0x0F | 0x10 => {
                // Write Multiple (coils/registers) requests include a byte count at offset 6
                if col.len() < 7 {
                    return None; // need more bytes to read byte-count
                }
                let bytecount = col[6] as usize;
                // total = id(1) + func(1) + addr2 + qty2 + bytecount(1) + data + crc2 = 9 + bytecount
                Some(9 + bytecount)
            }
            _ => None,
        }
    };

    // Try to read up to 6 bytes quickly so parse_modbus_header can run for response-style frames.
    // Try to read up to 6 bytes quickly so parse_modbus_header can run for response-style frames.
    read_until(&mut **guard, &mut collected, 6)?;

    // Try to determine the full expected frame length now; if not determined, wait until deadline
    let mut guessed_len_opt = determine_length(&mut collected);
    while guessed_len_opt.is_none() {
        if chrono::Utc::now() - start > Duration::seconds(2) {
            if collected.is_empty() {
                return Ok(None);
            } else {
                return Ok(Some(Bytes::from(collected)));
            }
        }
        // attempt to read one more byte to progress for functions that need more header
        // try to read up to 1 more byte
        let target = collected.len() + 1;
        read_until(&mut **guard, &mut collected, target)?;
        // Use async sleep instead of blocking sleep
        sleep_1s().await;
        guessed_len_opt = determine_length(&mut collected);
    }
    let guessed_len = guessed_len_opt.unwrap();
    // Modbus RTU maximum 256 bytes
    if !(4..=256).contains(&guessed_len) {
        log::warn!("Guessed invalid frame length: {guessed_len}");
        if collected.is_empty() {
            return Ok(None);
        } else {
            return Ok(Some(Bytes::from(collected)));
        }
    }
    // Read the remaining bytes with a 3 s additional deadline
    let body_deadline = chrono::Utc::now() + Duration::seconds(3);
    while collected.len() < guessed_len {
        // guessed_len includes header
        read_until(&mut **guard, &mut collected, guessed_len)?;
        if collected.len() >= guessed_len {
            break;
        }
        if chrono::Utc::now() > body_deadline {
            log::warn!(
                "Frame read timeout (expected {guessed_len}, got {})",
                collected.len()
            );
            if collected.is_empty() {
                return Ok(None);
            } else {
                return Ok(Some(Bytes::from(collected)));
            }
        }
        // Use async sleep instead of blocking sleep
        sleep_1s().await;
    }

    if collected.len() != guessed_len {
        if collected.is_empty() {
            return Ok(None);
        } else {
            return Ok(Some(Bytes::from(collected)));
        }
    }

    // CRC check (little endian stored: low then high)
    if guessed_len >= 4 {
        let data_no_crc_len = guessed_len - 2;
        let calc = crc16_modbus(&collected[..data_no_crc_len]);
        let frame_crc =
            (collected[data_no_crc_len] as u16) | ((collected[data_no_crc_len + 1] as u16) << 8);
        if calc != frame_crc {
            log::warn!("CRC mismatch: calc=0x{calc:04X} frame=0x{frame_crc:04X}");
            if collected.is_empty() {
                return Ok(None);
            } else {
                return Ok(Some(Bytes::from(collected)));
            }
        }
    }

    // Flush serial output buffers (optional safety)
    // flush using the same guard
    guard.flush()?;

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
