use anyhow::Result;
use flume::{Receiver, Sender};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serialport::SerialPort;

use crate::runtime::{RuntimeCommand, RuntimeEvent, SerialConfig};

// Read buffer and assembling limits:
// - READ_BUF_SIZE: size of the temporary read buffer used for each serial::read call.
//   This is independent of the assembling buffer and chosen to be large enough to
//   accommodate bursts from the serial driver (256 bytes).
// - MAX_ASSEMBLING_LEN: defensive upper bound for the assembling buffer. If the
//   code accumulates more than this many bytes without finding valid Modbus frames,
//   the buffer is finalized and cleared to avoid unbounded memory growth.
//   768 was chosen as 3 * READ_BUF_SIZE which gives room for a few consecutive
//   reads before deciding the stream is malformed or unsalvageable.
const READ_BUF_SIZE: usize = 256;
const MAX_ASSEMBLING_LEN: usize = 768; // 3 * READ_BUF_SIZE

/// Boot the serial port I/O loop
/// Must be started in a separate thread, otherwise it will block the main thread
pub fn boot_serial_loop(
    serial: Arc<Mutex<Box<dyn SerialPort + Send + 'static>>>,
    port_name: String,
    initial: SerialConfig,
    cmd_rx: Receiver<RuntimeCommand>,
    evt_tx: Sender<RuntimeEvent>,
) -> Result<()> {
    let mut gap = compute_gap(&initial);
    let mut assembling: Vec<u8> = Vec::with_capacity(READ_BUF_SIZE);
    let mut last_byte: Option<Instant> = None;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                RuntimeCommand::Reconfigure(new_cfg) => {
                    if let Err(err) = reopen_serial(&serial, &port_name, &new_cfg) {
                        evt_tx.send(RuntimeEvent::Error(format!("Reconfigure failed: {err}")))?;
                    } else {
                        gap = compute_gap(&new_cfg);
                        evt_tx.send(RuntimeEvent::Reconfigured(new_cfg))?;
                    }
                }
                RuntimeCommand::Write(bytes) => {
                    let mut ok = false;
                    if let Ok(mut serial) = serial.lock() {
                        if serial.write_all(&bytes).is_ok() && serial.flush().is_ok() {
                            ok = true;
                        }
                    }
                    if ok {
                        evt_tx.send(RuntimeEvent::FrameSent(bytes.into()))?;
                    }
                }
                RuntimeCommand::Stop => {
                    evt_tx.send(RuntimeEvent::Stopped)?;
                    return Ok(());
                }
            }
        }
        if let Some(t) = last_byte {
            if !assembling.is_empty() && t.elapsed() >= gap {
                finalize_buffer(assembling.as_mut_slice(), &evt_tx)?;
                assembling.clear();
                last_byte = None;
            }
        }
        if let Ok(mut g) = serial.lock() {
            let mut buf = [0u8; READ_BUF_SIZE];
            match g.read(&mut buf) {
                Ok(n) => {
                    if n > 0 {
                        assembling.extend_from_slice(&buf[..n]);
                        last_byte = Some(Instant::now());
                        if assembling.len() > MAX_ASSEMBLING_LEN {
                            finalize_buffer(assembling.as_mut_slice(), &evt_tx)?;
                            assembling.clear();
                            last_byte = None;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(err) => {
                    evt_tx.send(RuntimeEvent::Error(format!("Read error: {err}")))?;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn compute_gap(cfg: &SerialConfig) -> Duration {
    let bit_time_us = 1_000_000u64 / cfg.baud as u64;
    let bits_per_byte = cfg.data_bits as u64 + cfg.stop_bits as u64 + 1u64; // +1 for start bit
    let byte_time_us = bit_time_us * bits_per_byte;
    Duration::from_micros(byte_time_us.saturating_mul(35).saturating_div(10))
}

fn finalize_buffer(buffer: &mut [u8], evt_tx: &Sender<RuntimeEvent>) -> Result<()> {
    if !buffer.is_empty() {
        evt_tx.send(RuntimeEvent::FrameReceived(buffer.to_owned().into()))?;
    }
    Ok(())
}

fn reopen_serial(
    serial: &Arc<Mutex<Box<dyn SerialPort + Send + 'static>>>,
    port_name: &str,
    cfg: &SerialConfig,
) -> Result<()> {
    if let Ok(mut serial) = serial.lock() {
        let mut builder = serialport::new(port_name, cfg.baud);
        builder = cfg.apply_builder(builder);
        let new_port = builder.open()?;
        *serial = new_port;
    }
    Ok(())
}
