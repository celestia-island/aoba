use anyhow::Result;
use flume::{Receiver, Sender};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serialport::{DataBits, SerialPort, StopBits};

// Read buffer and assembling limits shared by runtime implementation.
const READ_BUF_SIZE: usize = 256;
const MAX_ASSEMBLING_LEN: usize = 768; // defensive cap (3 * READ_BUF_SIZE)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: serialport::Parity,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            baud: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: serialport::Parity::None,
        }
    }
}

impl SerialConfig {
    pub fn apply_builder(&self, b: serialport::SerialPortBuilder) -> serialport::SerialPortBuilder {
        let b = b.data_bits(match self.data_bits {
            5 => DataBits::Five,
            6 => DataBits::Six,
            7 => DataBits::Seven,
            _ => DataBits::Eight,
        });
        let b = b.stop_bits(match self.stop_bits {
            2 => StopBits::Two,
            _ => StopBits::One,
        });

        b.parity(self.parity)
    }
}

#[derive(Debug)]
pub enum RuntimeCommand {
    Reconfigure(SerialConfig),
    Write(Vec<u8>),
    Stop,
}
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    FrameReceived(bytes::Bytes),
    FrameSent(bytes::Bytes),
    Reconfigured(SerialConfig),
    Error(String),
    Stopped,
}

#[derive(Clone)]
pub struct PortRuntimeHandle {
    pub cmd_tx: Sender<RuntimeCommand>,
    pub evt_rx: Receiver<RuntimeEvent>,
    pub current_cfg: SerialConfig,
    pub shared_serial: Arc<Mutex<Box<dyn SerialPort + Send + 'static>>>,
    pub thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<Result<()>>>>>,
}

impl std::fmt::Debug for PortRuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortRuntimeHandle")
            .field("current_cfg", &self.current_cfg)
            .finish_non_exhaustive()
    }
}

impl PortRuntimeHandle {
    pub fn spawn(port_name: String, initial: SerialConfig) -> Result<Self> {
        let builder =
            serialport::new(port_name.clone(), initial.baud).timeout(Duration::from_millis(200));
        let builder = initial.apply_builder(builder);
        let handle = builder.open()?;
        let serial: Arc<Mutex<Box<dyn SerialPort + Send + 'static>>> = Arc::new(Mutex::new(handle));
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, evt_rx) = flume::unbounded();
        let serial_clone = Arc::clone(&serial);
        let initial_cfg = initial.clone();

        let handle = std::thread::spawn(move || {
            boot_serial_loop(serial_clone, port_name, initial_cfg, cmd_rx, evt_tx)
        });
        Ok(Self {
            cmd_tx,
            evt_rx,
            current_cfg: initial,
            shared_serial: serial,
            thread_handle: Arc::new(Mutex::new(Some(handle))),
        })
    }

    pub fn from_existing(
        handle: Box<dyn SerialPort + Send + 'static>,
        initial: SerialConfig,
    ) -> Result<Self> {
        let serial: Arc<Mutex<Box<dyn SerialPort + Send + 'static>>> = Arc::new(Mutex::new(handle));
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, evt_rx) = flume::unbounded();
        let serial_clone = Arc::clone(&serial);
        let initial_cfg = initial.clone();

        let handle = std::thread::spawn(move || {
            crate::protocol::daemon::boot_serial_loop(
                serial_clone,
                String::new(),
                initial_cfg,
                cmd_rx,
                evt_tx,
            )
        });
        Ok(Self {
            cmd_tx,
            evt_rx,
            current_cfg: initial,
            shared_serial: serial,
            thread_handle: Arc::new(Mutex::new(Some(handle))),
        })
    }
}

impl Drop for PortRuntimeHandle {
    fn drop(&mut self) {
        if let Err(err) = self.cmd_tx.send(RuntimeCommand::Stop) {
            log::warn!("PortRuntimeHandle stop command send error: {err:?}");
        }

        if let Ok(mut thread) = self.thread_handle.lock() {
            if let Some(thread) = thread.take() {
                if let Err(err) = thread.join() {
                    log::warn!("PortRuntimeHandle thread join error: {err:?}");
                }
            }
        } else {
            log::warn!("PortRuntimeHandle failed to lock thread_handle for join");
        }
    }
}

/// Boot the serial port I/O loop
/// Must be started in a separate thread, otherwise it will block the main thread
fn boot_serial_loop(
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
                finalize_buffer(&mut assembling, &evt_tx)?;
                assembling.clear();
                last_byte = None;
            }
        }
        if let Ok(mut g) = serial.lock() {
            let mut buf = [0u8; READ_BUF_SIZE];
            match g.read(&mut buf) {
                Ok(n) if n > 0 => {
                    assembling.extend_from_slice(&buf[..n]);
                    last_byte = Some(Instant::now());
                    if assembling.len() > MAX_ASSEMBLING_LEN {
                        finalize_buffer(&mut assembling, &evt_tx)?;
                        assembling.clear();
                        last_byte = None;
                    }
                }
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(err) => {
                    evt_tx.send(RuntimeEvent::Error(format!("read error: {err}")))?;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

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

fn candidate_lengths(b: &[u8]) -> Vec<usize> {
    let mut v = Vec::new();
    if b.len() < 2 {
        return v;
    }
    let f = b[1];
    if (f & 0x80) != 0 {
        v.push(5);
        return v;
    }
    match f {
        0x01..=0x04 => {
            v.push(8);
            if b.len() >= 3 {
                let bc = b[2] as usize;
                let rl = 3 + bc + 2;
                if (5..=256).contains(&rl) {
                    v.push(rl);
                }
            }
        }
        0x05 | 0x06 => v.push(8),
        0x0F | 0x10 => {
            if b.len() >= 7 {
                let bc = b[6] as usize;
                let req = 9 + bc;
                if req <= 256 {
                    v.push(req);
                }
            }
            v.push(8);
        }
        _ => {}
    }
    v
}

fn salvage_search(buf: &[u8]) -> Option<(usize, usize)> {
    if buf.len() < 5 {
        return None;
    }
    for s in 0..buf.len().saturating_sub(4) {
        let slice = &buf[s..];
        let c = candidate_lengths(slice);
        for len in c {
            if len < 5 || len > slice.len() {
                continue;
            }
            let pl = len - 2;
            if pl >= slice.len() {
                continue;
            }
            let calc = crc16_modbus(&slice[..pl]);
            let crc = (slice[pl] as u16) | ((slice[pl + 1] as u16) << 8);
            if calc == crc {
                return Some((s, len));
            }
        }
    }
    None
}

fn finalize_residual(res: &mut Vec<u8>, out: &mut Vec<bytes::Bytes>) {
    if res.is_empty() {
        return;
    }
    let mut consumed = 0usize;
    let mut cur = &res[..];
    let mut salv = false;
    while !cur.is_empty() {
        if cur.len() < 5 {
            break;
        }
        let c = candidate_lengths(cur);
        let mut matched = false;
        for len in c {
            if len > cur.len() || len < 5 {
                continue;
            }
            let pl = len - 2;
            let calc = crc16_modbus(&cur[..pl]);
            let crc = (cur[pl] as u16) | ((cur[pl + 1] as u16) << 8);
            if calc == crc {
                out.push(bytes::Bytes::from(cur[..len].to_vec()));
                consumed += len;
                cur = &cur[len..];
                matched = true;
                break;
            }
        }
        if !matched {
            if !salv {
                if let Some((s, l)) = salvage_search(cur) {
                    if s > 0 {
                        consumed += s;
                    }
                    out.push(bytes::Bytes::from(cur[s..s + l].to_vec()));
                    consumed += l - s;
                    cur = &cur[s + l..];
                    salv = true;
                    continue;
                }
            }
            break;
        }
    }
    if consumed > 0 {
        res.drain(0..consumed);
    } else {
        res.clear();
    }
}

fn finalize_buffer(buf: &mut Vec<u8>, evt: &Sender<RuntimeEvent>) -> Result<()> {
    let mut frames = Vec::new();
    finalize_residual(buf, &mut frames);
    if frames.is_empty() {
        if log::log_enabled!(log::Level::Debug) {
            let hex = buf
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            log::debug!("finalize: no frame len={} hex={}", buf.len(), hex);
        }
    } else {
        for frame in frames {
            evt.send(RuntimeEvent::FrameReceived(frame))?;
        }
    }

    Ok(())
}

fn compute_gap(cfg: &SerialConfig) -> Duration {
    let bits = 1.
        + cfg.data_bits as f32
        + (if cfg.parity != serialport::Parity::None {
            1.
        } else {
            0.
        })
        + cfg.stop_bits as f32;
    let char_ms = (bits / cfg.baud as f32) * 1000.;
    let gap_ms = (char_ms * 4.).clamp(3., 50.);
    Duration::from_millis(gap_ms as u64)
}

fn reopen_serial(
    shared: &Arc<Mutex<Box<dyn SerialPort + Send + 'static>>>,
    port: &str,
    cfg: &SerialConfig,
) -> Result<()> {
    if port.is_empty() {
        return Ok(());
    } // from_existing path leaves port_name empty
    let builder = serialport::new(port, cfg.baud).timeout(Duration::from_millis(200));
    let builder = cfg.apply_builder(builder);
    let new_handle = builder.open()?;
    if let Ok(mut handle) = shared.lock() {
        *handle = new_handle;
    }
    Ok(())
}
