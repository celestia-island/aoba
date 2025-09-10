use chrono::{DateTime, Local};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use yuuka::derive_struct;

use rmodbus::server::storage::ModbusStorageSmall;
use serialport::SerialPortInfo;

use crate::protocol::{
    runtime::PortRuntimeHandle,
    status::{LogEntry, SubpageForm},
    tty::PortExtra,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Ascii,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubpageTab {
    Config = 0,
    Body = 1,
    Log = 2,
}

impl SubpageTab {
    /// Return the numeric index for UI widgets that require usize.
    pub fn as_usize(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeOverlayIndex {
    Modbus = 0,
    Mqtt = 1,
}

impl ModeOverlayIndex {
    /// Return the numeric index for UI widgets that require usize.
    pub fn as_usize(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Modbus,
    Mqtt,
}

derive_struct! {
    pub Status {
        ports: {
            list: Vec<SerialPortInfo>,
            extras: Vec<PortExtra>,
            states: Vec<crate::protocol::status::PortState>,
            handles: Vec<Option<crate::protocol::status::SerialPortWrapper>>,
            runtimes: Vec<Option<PortRuntimeHandle>>,
            about_view_offset: usize,
        },
        ui: {
            selected: usize,
            auto_refresh: bool,
            last_refresh: Option<DateTime<Local>>,
            error: Option<(String, DateTime<Local>)>,
            subpage_active: bool,
            subpage_form: Option<SubpageForm>,
            subpage_tab_index: SubpageTab = SubpageTab::Config,
            logs: Vec<LogEntry>,
            log_selected: usize,
            log_view_offset: usize,
            log_auto_scroll: bool,
            log_clear_pending: bool,
            input_mode: InputMode = InputMode::Ascii,
            input_editing: bool,
            input_buffer: String,
            app_mode: AppMode = AppMode::Modbus,
            mode_overlay_active: bool,
            mode_overlay_index: ModeOverlayIndex = ModeOverlayIndex::Modbus,
            pages: [enum Page {
                Entry {
                    selected: usize,
                    input_mode: InputMode,
                    input_editing: bool,
                    input_buffer: String,
                    app_mode: AppMode,
                },
                Modbus {
                    // TODO: Subpage level enum
                    selected: usize,
                    subpage_active: bool,
                    subpage_form: Option<SubpageForm>,
                    subpage_tab_index: SubpageTab,
                    logs: Vec<LogEntry>,
                    log_selected: usize,
                    log_view_offset: usize,
                    log_auto_scroll: bool,
                    log_clear_pending: bool,
                    input_mode: InputMode,
                    input_editing: bool,
                    input_buffer: String,
                    app_mode: AppMode,
                },
            }] = vec![Page::Entry {
                selected: 0,
                input_mode: InputMode::Ascii,
                input_editing: false,
                input_buffer: String::new(),
                app_mode: AppMode::Modbus,
            }],
        },
        per_port: {
            states: HashMap<String, crate::protocol::status::PerPortState>,
            slave_contexts: HashMap<String, Arc<Mutex<ModbusStorageSmall>>>,
            pending_sync_port: Option<String>,
        },
        scan: {
            last_scan_info: Vec<String>,
            last_scan_time: Option<DateTime<Local>>,
        },
        busy: {
            busy: bool = false,
            spinner_frame: u8 = 0,
            polling_paused: bool = false,
        },
        toggles: {
            last_port_toggle: Option<std::time::Instant>,
            port_toggle_min_interval_ms: u64 = crate::protocol::status::PORT_TOGGLE_MIN_INTERVAL_MS,
        },
        recent: {
            auto_sent: VecDeque<(Vec<u8>, std::time::Instant)>,
            auto_requests: VecDeque<(Vec<u8>, std::time::Instant)>,
        },
    }
}
