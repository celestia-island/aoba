use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::Result;
use flume::{Receiver, Sender};
use serialport::{DataBits, SerialPort, StopBits};

use crate::protocol::modbus::read_modbus_frame;

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
        let b = b.parity(self.parity);
        b
    }
}

#[derive(Debug)]
pub enum RuntimeCommand {
    Reconfigure(SerialConfig),
    Write(Vec<u8>),
    Stop,
}

#[derive(Debug)]
pub enum RuntimeEvent {
    FrameReceived(bytes::Bytes),
    FrameSent(bytes::Bytes),
    Reconfigured(SerialConfig),
    Error(String),
    Stopped,
}

pub struct PortRuntimeHandle {
    pub cmd_tx: Sender<RuntimeCommand>,
    pub evt_rx: Receiver<RuntimeEvent>,
    pub current_cfg: SerialConfig,
    pub shared_serial: Arc<Mutex<Box<dyn SerialPort + Send + 'static>>>,
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
        thread::spawn(move || {
            let mut last_frame_check = std::time::Instant::now();
            loop {
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        RuntimeCommand::Reconfigure(cfg) => {
                            if let Err(e) = reopen_serial(&serial_clone, &port_name, &cfg) {
                                let _ = evt_tx
                                    .send(RuntimeEvent::Error(format!("reconfigure failed: {e}")));
                            } else {
                                let _ = evt_tx.send(RuntimeEvent::Reconfigured(cfg));
                            }
                        }
                        RuntimeCommand::Write(bytes) => {
                            let mut ok = false;
                            if let Ok(mut guard) = serial_clone.lock() {
                                use std::io::Write;
                                if guard.write_all(&bytes).is_ok() && guard.flush().is_ok() {
                                    ok = true;
                                }
                            }
                            if ok {
                                let _ = evt_tx.send(RuntimeEvent::FrameSent(bytes.into()));
                            }
                        }
                        RuntimeCommand::Stop => {
                            let _ = evt_tx.send(RuntimeEvent::Stopped);
                            return;
                        }
                    }
                }
                if last_frame_check.elapsed() >= Duration::from_millis(50) {
                    last_frame_check = std::time::Instant::now();
                    match read_modbus_frame(Arc::clone(&serial_clone)) {
                        Ok(Some(frame)) => {
                            let _ = evt_tx.send(RuntimeEvent::FrameReceived(frame));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = evt_tx.send(RuntimeEvent::Error(format!("read error: {e}")));
                        }
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        Ok(Self {
            cmd_tx,
            evt_rx,
            current_cfg: initial,
            shared_serial: serial,
        })
    }

    /// Use an already-opened SerialPort handle (avoid reopening it a second time which could cause a resource conflict)
    pub fn from_existing(
        handle: Box<dyn SerialPort + Send + 'static>,
        initial: SerialConfig,
    ) -> Result<Self> {
        let serial: Arc<Mutex<Box<dyn SerialPort + Send + 'static>>> = Arc::new(Mutex::new(handle));
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, evt_rx) = flume::unbounded();
        let serial_clone = Arc::clone(&serial);
        // No need to reopen here; just spawn the read thread using the existing handle
        thread::spawn(move || {
            let mut last_frame_check = std::time::Instant::now();
            loop {
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        RuntimeCommand::Reconfigure(cfg) => {
                            if let Err(e) = reopen_serial(&serial_clone, "", &cfg) {
                                let _ = evt_tx
                                    .send(RuntimeEvent::Error(format!("reconfigure failed: {e}")));
                            } else {
                                let _ = evt_tx.send(RuntimeEvent::Reconfigured(cfg));
                            }
                        }
                        RuntimeCommand::Write(bytes) => {
                            let mut ok = false;
                            if let Ok(mut guard) = serial_clone.lock() {
                                use std::io::Write;
                                if guard.write_all(&bytes).is_ok() && guard.flush().is_ok() {
                                    ok = true;
                                }
                            }
                            if ok {
                                let _ = evt_tx.send(RuntimeEvent::FrameSent(bytes.into()));
                            }
                        }
                        RuntimeCommand::Stop => {
                            let _ = evt_tx.send(RuntimeEvent::Stopped);
                            return;
                        }
                    }
                }
                if last_frame_check.elapsed() >= Duration::from_millis(50) {
                    last_frame_check = std::time::Instant::now();
                    match read_modbus_frame(Arc::clone(&serial_clone)) {
                        Ok(Some(frame)) => {
                            let _ = evt_tx.send(RuntimeEvent::FrameReceived(frame));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = evt_tx.send(RuntimeEvent::Error(format!("read error: {e}")));
                        }
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        Ok(Self {
            cmd_tx,
            evt_rx,
            current_cfg: initial,
            shared_serial: serial,
        })
    }
}

fn reopen_serial(
    shared: &Arc<Mutex<Box<dyn SerialPort + Send + 'static>>>,
    port_name: &str,
    cfg: &SerialConfig,
) -> Result<()> {
    let builder = serialport::new(port_name, cfg.baud).timeout(Duration::from_millis(200));
    let builder = cfg.apply_builder(builder);
    let new_handle = builder.open()?;
    if let Ok(mut guard) = shared.lock() {
        *guard = new_handle;
    }
    Ok(())
}
