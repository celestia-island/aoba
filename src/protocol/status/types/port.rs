use chrono::{DateTime, Local};
use serialport::SerialPortInfo;
use std::sync::{Arc, Mutex};

use crate::protocol::{runtime::PortRuntimeHandle, tty::PortExtra};

/// A single log entry associated with a specific port.
#[derive(Debug, Clone)]
pub struct PortLogEntry {
    pub when: DateTime<Local>,
    pub raw: String,
    pub parsed: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PortState {
    Free,
    OccupiedByThis {
        handle: Option<SerialPortWrapper>,
        runtime: PortRuntimeHandle,
    },
    OccupiedByOther,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct SerialPortWrapper(Arc<Mutex<Box<dyn serialport::SerialPort + Send>>>);

impl std::fmt::Debug for SerialPortWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SerialPortWrapper").finish()
    }
}

impl SerialPortWrapper {
    pub fn new(inner: Arc<Mutex<Box<dyn serialport::SerialPort + Send>>>) -> Self {
        SerialPortWrapper(inner)
    }
}

#[derive(Debug, Clone)]
pub struct PortData {
    pub port_name: String,
    pub port_type: String,
    pub info: Option<SerialPortInfo>,
    pub extra: PortExtra,
    pub state: PortState,

    pub logs: Vec<PortLogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub log_clear_pending: bool,
}

impl Default for PortData {
    fn default() -> Self {
        PortData {
            port_name: String::new(),
            port_type: String::new(),
            info: None,
            extra: Default::default(),
            state: PortState::Free,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            log_clear_pending: false,
        }
    }
}
