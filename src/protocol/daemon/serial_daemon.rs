use anyhow::Result;
use flume::{Receiver, Sender};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serialport::{DataBits, SerialPort, StopBits};

use crate::protocol::runtime::{RuntimeCommand, RuntimeEvent, SerialConfig};

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
    let mut assembling: Vec<u8> = Vec::with_capacity(256);
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
                finalize_buffer(&mut assembling, &evt_tx)?;
                assembling.clear();
                last_byte = None;
            }
        }
        if let Ok(mut g) = serial.lock() {
            let mut buf = [0u8; 256];
            match g.read(&mut buf) {
                Ok(n) if n > 0 => {
                    assembling.extend_from_slice(&buf[..n]);
                    last_byte = Some(Instant::now());
                    if assembling.len() > 768 {
                        finalize_buffer(&mut assembling, &evt_tx)?;
                        assembling.clear();
                        last_byte = None;
                    }
                }
                Ok(0) => {}
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    evt_tx.send(RuntimeEvent::Error(format!("Read error: {e}")))?;
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

fn finalize_buffer(buffer: &mut Vec<u8>, evt_tx: &Sender<RuntimeEvent>) -> Result<()> {
    if !buffer.is_empty() {
        evt_tx.send(RuntimeEvent::FrameReceived(buffer.clone().into()))?;
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