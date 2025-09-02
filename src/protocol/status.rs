use chrono::{DateTime, Local};
use std::cmp::{max, min};
use std::{collections::HashMap, time::Duration};

use serialport::new as sp_new;
use serialport::{Parity as SerialParity, SerialPort, SerialPortInfo, StopBits};

use crate::protocol::runtime::{PortRuntimeHandle, RuntimeCommand, RuntimeEvent, SerialConfig};
use crate::protocol::tty::available_ports_sorted;
use crate::tui::utils::constants::LOG_GROUP_HEIGHT;

/// Minimum interval (milliseconds) between consecutive port occupancy toggles to prevent OS driver self-lock.
pub const PORT_TOGGLE_MIN_INTERVAL_MS: u64 = 1500; // 1.5 seconds

/// Parsed summary of a captured protocol request / response for UI display.
#[derive(Debug, Clone)]
pub struct ParsedRequest {
    /// Origin of the message (e.g. "master-stack" or "main-stack")
    pub origin: String,
    /// "R" or "W"
    pub rw: String,
    /// Textual command or function code (e.g. "Read Coils / 0x01")
    pub command: String,
    pub slave_id: u8,
    pub address: u16,
    pub length: u16,
}

/// A single captured log entry for UI presentation.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub when: DateTime<Local>,
    /// Raw bytes or textual payload (displayed truncated)
    pub raw: String,
    /// Optional parsed summary
    pub parsed: Option<ParsedRequest>,
}

/// Input mode for the log input box: ASCII text or Hex bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Ascii,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterMode {
    Coils = 1,
    DiscreteInputs = 2,
    Holding = 3,
    Input = 4,
}

impl RegisterMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => Self::Coils,
        }
    }
    pub fn as_u8(self) -> u8 {
        self as u8
    }
    pub fn all() -> [RegisterMode; 4] {
        [
            Self::Coils,
            Self::DiscreteInputs,
            Self::Holding,
            Self::Input,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RegisterEntry {
    pub slave_id: u8,
    pub mode: RegisterMode,
    pub address: u16,
    pub length: u16,
    /// Placeholder register values for UI editing (length-sized, hex display)
    pub values: Vec<u8>,
    /// Refresh interval in milliseconds (for listen panel)
    pub refresh_ms: u32,
    /// Successful request count
    pub req_success: u64,
    /// Total request count
    pub req_total: u64,
    /// Next scheduled poll instant (monotonic) for active polling modes
    pub next_poll_at: std::time::Instant,
}

#[derive(Debug, Clone)]
pub struct SubpageForm {
    pub baud: u32,
    pub parity: Parity,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub registers: Vec<RegisterEntry>,
    // UI state
    pub cursor: usize, // Which field or register is focused
    pub editing: bool, // Whether in edit mode
    // Which specific field is being edited (None when not editing)
    pub editing_field: Option<EditingField>,
    // Input buffer for the current editing session (text)
    pub input_buffer: String,
    /// Temporary index used when editing a multi-option field (like Baud presets + Custom)
    pub edit_choice_index: Option<usize>,
    /// Whether we've entered the deeper confirm / editing stage for a choice (e.g. Custom baud)
    pub edit_confirmed: bool,
    // --- Master list (tab 1) dedicated UI state ---
    /// Cursor in master list panel (points to a master or the trailing "new" entry)
    pub master_cursor: usize,
    /// Whether currently editing a master entry (deprecated flag, kept for future use)
    pub master_editing: bool,
    /// Whether a specific master field is selected (field selection layer)
    pub master_field_selected: bool,
    /// Whether the current field is in active input editing (vs merely selected)
    pub master_field_editing: bool,
    /// Currently focused master field
    pub master_edit_field: Option<MasterEditField>,
    /// Input buffer for master field editing (hex / decimal)
    pub master_input_buffer: String,
    /// Index of the master being edited
    pub master_edit_index: Option<usize>,
}

impl Default for SubpageForm {
    fn default() -> Self {
        Self {
            baud: 9600,
            parity: Parity::None,
            data_bits: 8,
            stop_bits: 1,
            registers: vec![],
            cursor: 0,
            editing: false,
            editing_field: None,
            input_buffer: String::new(),
            edit_choice_index: None,
            edit_confirmed: false,
            master_cursor: 0,
            master_editing: false,
            master_field_selected: false,
            master_field_editing: false,
            master_edit_field: None,
            master_input_buffer: String::new(),
            master_edit_index: None,
        }
    }
}

/// Which concrete field inside the SubpageForm is currently being edited.
#[derive(Debug, Clone)]
pub enum EditingField {
    Baud,
    Parity,
    DataBits,
    StopBits,
    RegisterField { idx: usize, field: RegisterField },
}

#[derive(Debug, Clone)]
pub enum RegisterField {
    SlaveId,
    Mode,
    Address,
    Length,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MasterEditField {
    Id,
    Type,
    Start,
    End,
    /// Single register value (absolute address)
    Value(u16),
    /// Refresh interval (ms) for listen panel
    Refresh,
    /// Request counter (for reset action via Enter)
    Counter,
}

// Focus enum removed: UI now uses single-pane left list only

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortMode {
    Master,
    SlaveStack,
}

#[derive(Debug)]
pub struct Status {
    pub ports: Vec<SerialPortInfo>,
    /// Occupancy state for each port (same index as `ports`)
    pub port_states: Vec<PortState>,
    /// Optional open handle when this app occupies the port
    pub port_handles: Vec<Option<Box<dyn SerialPort>>>,
    /// Active runtime listeners per occupied port
    pub port_runtimes: Vec<Option<PortRuntimeHandle>>,
    pub selected: usize,
    pub auto_refresh: bool,
    pub last_refresh: Option<DateTime<Local>>,
    pub error: Option<(String, DateTime<Local>)>,
    pub port_mode: PortMode,
    /// When Some, a subpage for the right side is active (entered). None means main entry view.
    pub active_subpage: Option<PortMode>,
    /// Transient UI state for the active subpage (editable form)
    pub subpage_form: Option<SubpageForm>,
    /// Selected tab index inside the active right-side subpage
    pub subpage_tab_index: usize,
    /// Recent protocol / log entries (current working port)
    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
    /// Cached per-port UI states keyed by port name
    pub per_port_states: HashMap<String, PerPortState>,
    /// Transient mode selector overlay state (not per-port)
    pub mode_selector_active: bool,
    pub mode_selector_index: usize,
    /// Raw device tree info from last manual refresh (e.g., lsusb output on Linux)
    pub last_scan_info: Vec<String>,
    /// Timestamp of last manual refresh scan (device enumeration time)
    pub last_scan_time: Option<DateTime<Local>>,
    /// Whether a time‑consuming background mutation is in progress (mode switch / port reload etc.)
    pub busy: bool,
    /// Spinner frame index
    pub spinner_frame: u8,
    /// Whether slave listen polling is temporarily paused due to parameter editing
    pub polling_paused: bool,
    /// Last time a port occupancy toggle was performed (throttling rapid Enter presses)
    pub last_port_toggle: Option<std::time::Instant>,
    /// Minimum interval between port toggles
    pub port_toggle_min_interval_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PerPortState {
    pub port_mode: PortMode,
    pub active_subpage: Option<PortMode>,
    pub subpage_form: Option<SubpageForm>,
    pub subpage_tab_index: usize,
    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
}

impl Default for PerPortState {
    fn default() -> Self {
        Self {
            port_mode: PortMode::Master,
            active_subpage: None,
            subpage_form: None,
            subpage_tab_index: 0,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            input_mode: InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
}

impl Status {
    /// Platform specific device scan. Populates last_scan_info and updates last_scan_time.
    /// Shared by refresh() and quick_scan() to avoid duplication.
    fn perform_device_scan(&mut self) {
        self.last_scan_info.clear();
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            if let Ok(out) = Command::new("lsusb").output() {
                if out.status.success() {
                    if let Ok(text) = String::from_utf8(out.stdout) {
                        for line in text.lines() {
                            self.last_scan_info.push(line.to_string());
                        }
                    }
                } else if let Ok(err_text) = String::from_utf8(out.stderr) {
                    self.last_scan_info
                        .push(format!("ERROR: lsusb: {}", err_text.trim()));
                }
            } else {
                self.last_scan_info
                    .push("ERROR: lsusb invocation failed".to_string());
            }
        }
        #[cfg(target_os = "windows")]
        {
            use std::process::{Command, Stdio};
            use std::thread;
            use std::time::Duration as StdDuration;
            // Run powershell in a helper thread so we can implement a simple timeout.
            let (tx, rx) = std::sync::mpsc::channel();
            thread::spawn(move || {
                let ps_cmd = [
                    "-NoLogo",
                    "-NoProfile",
                    "-Command",
                    "Get-PnpDevice -Class Ports | Select-Object -Property FriendlyName,InstanceId | Format-Table -HideTableHeaders",
                ];
                let result = Command::new("powershell")
                    .args(&ps_cmd)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output();
                let _ = tx.send(result);
            });
            let mut any_success = false;
            if let Ok(res) = rx.recv_timeout(StdDuration::from_millis(1500)) {
                match res {
                    Ok(out) => {
                        if out.status.success() {
                            if let Ok(text) = String::from_utf8(out.stdout) {
                                for line in text.lines() {
                                    if !line.trim().is_empty() {
                                        self.last_scan_info.push(line.trim().to_string());
                                    }
                                }
                                any_success = true;
                            }
                        } else if let Ok(err_text) = String::from_utf8(out.stderr) {
                            self.last_scan_info.push(format!(
                                "ERROR: powershell Get-PnpDevice: {}",
                                err_text.trim()
                            ));
                        }
                    }
                    Err(e) => {
                        self.last_scan_info
                            .push(format!("ERROR: powershell exec error: {e}"));
                    }
                }
            } else {
                self.last_scan_info
                    .push("ERROR: powershell scan timeout".to_string());
            }
            if !any_success {
                if let Ok(out) = Command::new("wmic")
                    .args(["path", "Win32_SerialPort", "get", "Name,DeviceID"])
                    .output()
                {
                    if out.status.success() {
                        if let Ok(text) = String::from_utf8(out.stdout) {
                            for line in text.lines() {
                                if !line.trim().is_empty() {
                                    self.last_scan_info.push(line.trim().to_string());
                                }
                            }
                        }
                    } else if let Ok(err_text) = String::from_utf8(out.stderr) {
                        self.last_scan_info
                            .push(format!("ERROR: wmic Win32_SerialPort: {}", err_text.trim()));
                    }
                } else {
                    self.last_scan_info
                        .push("ERROR: failed to invoke wmic".to_string());
                }
            }
        }
        self.last_scan_time = Some(Local::now());
    }
    pub fn new() -> Self {
        let ports = available_ports_sorted();
        let port_states = Self::detect_port_states(&ports);
        let port_handles = ports.iter().map(|_| None).collect();
        let port_runtimes = ports.iter().map(|_| None).collect();
        Self {
            ports,
            port_states,
            port_handles,
            port_runtimes,
            selected: 0,

            auto_refresh: true,
            last_refresh: None,
            error: None,
            port_mode: PortMode::Master,
            active_subpage: None,
            subpage_form: None,
            subpage_tab_index: 0,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            input_mode: InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
            per_port_states: HashMap::new(),
            mode_selector_active: false,
            mode_selector_index: 0,
            last_scan_info: Vec::new(),
            last_scan_time: None,
            busy: false,
            spinner_frame: 0,
            polling_paused: false,
            last_port_toggle: None,
            port_toggle_min_interval_ms: PORT_TOGGLE_MIN_INTERVAL_MS,
        }
    }

    /// Adjust the log view window according to the current terminal height so the selected entry stays visible.
    pub fn adjust_log_view(&mut self, term_height: u16) {
        if self.logs.is_empty() {
            return;
        }
        let bottom_len = if self.error.is_some() || self.active_subpage.is_some() {
            2
        } else {
            1
        };
        // The constant 5 matches empirically reserved rows (title / input etc.) in current TUI layout; extract if layout changes.
        let logs_area_h = (term_height as usize).saturating_sub(bottom_len + 5);
        let inner_h = logs_area_h.saturating_sub(2);
        let groups_per_screen = max(1usize, inner_h / LOG_GROUP_HEIGHT);
        let bottom = if self.log_auto_scroll {
            self.logs.len().saturating_sub(1)
        } else {
            min(self.log_view_offset, self.logs.len().saturating_sub(1))
        };
        let top = if bottom + 1 >= groups_per_screen {
            bottom + 1 - groups_per_screen
        } else {
            0
        };
        if self.log_selected < top {
            self.log_auto_scroll = false;
            let half = groups_per_screen / 2;
            let new_bottom = min(self.logs.len().saturating_sub(1), self.log_selected + half);
            self.log_view_offset = new_bottom;
        } else if self.log_selected > bottom {
            self.log_auto_scroll = false;
            self.log_view_offset = self.log_selected;
        }
    }

    /// Page up in the log (decrease bottom index).
    pub fn page_up(&mut self, page: usize) {
        if self.logs.is_empty() {
            return;
        }
        if self.log_view_offset > page {
            self.log_view_offset = self.log_view_offset.saturating_sub(page);
        } else {
            self.log_view_offset = 0;
        }
        self.log_auto_scroll = false;
    }

    /// Page down in the log (increase bottom index).
    pub fn page_down(&mut self, page: usize) {
        if self.logs.is_empty() {
            return;
        }
        let total = self.logs.len();
        self.log_view_offset = min(total - 1, self.log_view_offset.saturating_add(page));
        self.log_auto_scroll = false;
    }

    /// Initialize subpage_form from selected port when entering a subpage
    pub fn init_subpage_form(&mut self) {
        // If no ports or selected is virtual, create a default form
        if self.ports.is_empty() || self.selected >= self.ports.len() {
            self.subpage_form = Some(SubpageForm::default());
            return;
        }
        // Try to populate from existing open handle if available
        let mut form = SubpageForm::default();
        if let Some(slot) = self.port_handles.get(self.selected) {
            if let Some(handle) = slot.as_ref() {
                form.baud = handle.baud_rate().unwrap_or(9600);
                form.stop_bits = handle
                    .stop_bits()
                    .map(|s| match s {
                        StopBits::One => 1,
                        StopBits::Two => 2,
                    })
                    .unwrap_or(1);
                // Parity mapping
                if let Ok(p) = handle.parity() {
                    form.parity = match p {
                        SerialParity::None => Parity::None,
                        SerialParity::Even => Parity::Even,
                        SerialParity::Odd => Parity::Odd,
                    };
                }
            }
        }
        self.subpage_form = Some(form);
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some((msg.into(), Local::now()));
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Create an App with provided ports (useful for tests)
    #[cfg(test)]
    pub fn with_ports(ports: Vec<SerialPortInfo>) -> Self {
        let port_states = ports.iter().map(|_| PortState::Free).collect();
        let port_handles = ports.iter().map(|_| None).collect();
        let port_runtimes = ports.iter().map(|_| None).collect();
        Self {
            ports,
            port_states,
            port_handles,
            port_runtimes,
            selected: 0,

            auto_refresh: false,
            last_refresh: None,
            error: None,
            port_mode: PortMode::Master,
            active_subpage: None,
            subpage_form: None,
            subpage_tab_index: 0,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            input_mode: InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
            per_port_states: HashMap::new(),
            mode_selector_active: false,
            mode_selector_index: 0,
            last_scan_info: Vec::new(),
            last_scan_time: None,
            busy: false,
            spinner_frame: 0,
            polling_paused: false,
            last_port_toggle: None,
            port_toggle_min_interval_ms: PORT_TOGGLE_MIN_INTERVAL_MS,
        }
    }

    /// Append a log entry to the internal buffer (caps at 1000 entries)
    pub fn append_log(&mut self, entry: LogEntry) {
        const MAX: usize = 1000;
        self.logs.push(entry);
        if self.logs.len() > MAX {
            let excess = self.logs.len() - MAX;
            self.logs.drain(0..excess);
            // Ensure selected index remains valid
            if self.log_selected >= self.logs.len() {
                self.log_selected = self.logs.len().saturating_sub(1);
            }
        }
        // Maintain auto-scroll behaviour: when auto-scroll enabled, keep view anchored to the latest
        if self.log_auto_scroll {
            // Position the view offset so bottom aligns with last entry (we'll compute exact top in renderer)
            if self.logs.is_empty() {
                self.log_view_offset = 0;
            } else {
                self.log_view_offset = self.logs.len().saturating_sub(1);
                // If auto-scroll is enabled, also move the selection to the newest entry
                self.log_selected = self.logs.len().saturating_sub(1);
            }
        }
    }

    /// Re-scan available ports and reset selection if needed
    pub fn refresh(&mut self) {
        self.busy = true;
        self.save_current_port_state();
        self.perform_device_scan();
        let new_ports = available_ports_sorted();
        // Remember previously selected port name (if any real port selected)
        let prev_selected_name = if !self.ports.is_empty() && self.selected < self.ports.len() {
            Some(self.ports[self.selected].port_name.clone())
        } else {
            None
        };
        // Preserve known states, handles and runtimes by port name
        let mut name_to_state: HashMap<String, PortState> = HashMap::new();
        let mut name_to_handle: HashMap<String, Option<Box<dyn SerialPort>>> = HashMap::new();
        let mut name_to_runtime: HashMap<String, Option<PortRuntimeHandle>> = HashMap::new();
        for (i, p) in self.ports.iter().enumerate() {
            if let Some(s) = self.port_states.get(i) {
                name_to_state.insert(p.port_name.clone(), *s);
            }
            // Take ownership of existing handle if any
            if let Some(h) = self.port_handles.get_mut(i) {
                let taken = h.take();
                name_to_handle.insert(p.port_name.clone(), taken);
            }
            // Preserve runtime so serial parameters remain visible after refresh
            if let Some(r) = self.port_runtimes.get_mut(i) {
                let taken = r.take();
                name_to_runtime.insert(p.port_name.clone(), taken);
            }
        }
        self.ports = new_ports;
        // Rebuild port_states and port_handles preserving by name
        let mut new_states: Vec<PortState> = Vec::with_capacity(self.ports.len());
        let mut new_handles: Vec<Option<Box<dyn SerialPort>>> =
            Vec::with_capacity(self.ports.len());
        for p in self.ports.iter() {
            if let Some(s) = name_to_state.remove(&p.port_name) {
                new_states.push(s);
            } else if Self::is_port_free(&p.port_name) {
                new_states.push(PortState::Free);
            } else {
                new_states.push(PortState::OccupiedByOther);
            }
            // Move back handle if existed
            if let Some(h) = name_to_handle.remove(&p.port_name) {
                new_handles.push(h);
            } else {
                new_handles.push(None);
            }
        }
        self.port_states = new_states;
        self.port_handles = new_handles;
        // Rebuild runtimes preserving by name
        self.port_runtimes = self
            .ports
            .iter()
            .map(|p| name_to_runtime.remove(&p.port_name).unwrap_or(None))
            .collect();
        // Stop any runtimes whose ports disappeared
        for (_name, rt_opt) in name_to_runtime.into_iter() {
            if let Some(rt) = rt_opt {
                let _ = rt.cmd_tx.send(RuntimeCommand::Stop);
            }
        }
        if self.ports.is_empty() {
            // No real ports -> reset selection to 0 (no virtual items rendered)
            self.selected = 0;
        } else {
            // Try to restore previous selected port by name
            if let Some(name) = prev_selected_name {
                if let Some(idx) = self.ports.iter().position(|p| p.port_name == name) {
                    self.selected = idx;
                }
            }
            // Ensure selected is within allowed range: real ports + 2 virtual items
            let total = self.ports.len().saturating_add(2);
            if self.selected >= total {
                self.selected = 0;
            }
        }
        self.last_refresh = Some(Local::now());
        self.load_current_port_state();
        self.busy = false;
    }

    /// Lightweight periodic refresh: only re-enumerate serial ports and occupancy state; skip external device scan to avoid stalls.
    pub fn refresh_ports_only(&mut self) {
        self.busy = true;
        self.save_current_port_state();
        let new_ports = available_ports_sorted();
        let prev_selected_name = if !self.ports.is_empty() && self.selected < self.ports.len() {
            Some(self.ports[self.selected].port_name.clone())
        } else {
            None
        };
        let mut name_to_state: HashMap<String, PortState> = HashMap::new();
        let mut name_to_handle: HashMap<String, Option<Box<dyn SerialPort>>> = HashMap::new();
        let mut name_to_runtime: HashMap<String, Option<PortRuntimeHandle>> = HashMap::new();
        for (i, p) in self.ports.iter().enumerate() {
            if let Some(s) = self.port_states.get(i) {
                name_to_state.insert(p.port_name.clone(), *s);
            }
            if let Some(h) = self.port_handles.get_mut(i) {
                let taken = h.take();
                name_to_handle.insert(p.port_name.clone(), taken);
            }
            if let Some(r) = self.port_runtimes.get_mut(i) {
                let taken = r.take();
                name_to_runtime.insert(p.port_name.clone(), taken);
            }
        }
        self.ports = new_ports;
        let mut new_states: Vec<PortState> = Vec::with_capacity(self.ports.len());
        let mut new_handles: Vec<Option<Box<dyn SerialPort>>> =
            Vec::with_capacity(self.ports.len());
        for p in self.ports.iter() {
            if let Some(s) = name_to_state.remove(&p.port_name) {
                new_states.push(s);
            } else if Self::is_port_free(&p.port_name) {
                new_states.push(PortState::Free);
            } else {
                new_states.push(PortState::OccupiedByOther);
            }
            if let Some(h) = name_to_handle.remove(&p.port_name) {
                new_handles.push(h);
            } else {
                new_handles.push(None);
            }
        }
        self.port_states = new_states;
        self.port_handles = new_handles;
        self.port_runtimes = self
            .ports
            .iter()
            .map(|p| name_to_runtime.remove(&p.port_name).unwrap_or(None))
            .collect();
        for (_name, rt_opt) in name_to_runtime.into_iter() {
            if let Some(rt) = rt_opt {
                let _ = rt.cmd_tx.send(RuntimeCommand::Stop);
            }
        }
        if self.ports.is_empty() {
            self.selected = 0;
        } else {
            if let Some(name) = prev_selected_name {
                if let Some(idx) = self.ports.iter().position(|p| p.port_name == name) {
                    self.selected = idx;
                }
            }
            let total = self.ports.len().saturating_add(2);
            if self.selected >= total {
                self.selected = 0;
            }
        }
        self.last_refresh = Some(Local::now());
        self.load_current_port_state();
        self.busy = false;
    }

    /// Quick device scan: update last_scan_info/time without re-enumerating serial ports
    pub fn quick_scan(&mut self) {
        self.perform_device_scan();
    }

    fn save_current_port_state(&mut self) {
        if self.selected < self.ports.len() {
            if let Some(info) = self.ports.get(self.selected) {
                let snap = PerPortState {
                    port_mode: self.port_mode,
                    active_subpage: self.active_subpage,
                    subpage_form: self.subpage_form.clone(),
                    subpage_tab_index: self.subpage_tab_index,
                    logs: self.logs.clone(),
                    log_selected: self.log_selected,
                    log_view_offset: self.log_view_offset,
                    log_auto_scroll: self.log_auto_scroll,
                    input_mode: self.input_mode,
                    input_editing: self.input_editing,
                    input_buffer: self.input_buffer.clone(),
                };
                self.per_port_states.insert(info.port_name.clone(), snap);
            }
        }
    }

    fn load_current_port_state(&mut self) {
        if self.selected < self.ports.len() {
            if let Some(info) = self.ports.get(self.selected) {
                if let Some(snap) = self.per_port_states.get(&info.port_name).cloned() {
                    self.port_mode = snap.port_mode;
                    self.active_subpage = snap.active_subpage;
                    self.subpage_form = snap.subpage_form;
                    self.subpage_tab_index = snap.subpage_tab_index;
                    self.logs = snap.logs;
                    self.log_selected = snap.log_selected;
                    self.log_view_offset = snap.log_view_offset;
                    self.log_auto_scroll = snap.log_auto_scroll;
                    self.input_mode = snap.input_mode;
                    self.input_editing = snap.input_editing;
                    self.input_buffer = snap.input_buffer;
                } else {
                    // Fresh defaults
                    self.port_mode = PortMode::Master;
                    self.active_subpage = None;
                    self.subpage_form = None;
                    self.subpage_tab_index = 0;
                    self.logs.clear();
                    self.log_selected = 0;
                    self.log_view_offset = 0;
                    self.log_auto_scroll = true;
                    self.input_mode = InputMode::Ascii;
                    self.input_editing = false;
                    self.input_buffer.clear();
                }
            }
        }
    }

    fn is_port_free(port_name: &str) -> bool {
        // Try to open the port briefly; if succeed it's free (we immediately drop it)
        match sp_new(port_name, 9600)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn detect_port_states(ports: &Vec<SerialPortInfo>) -> Vec<PortState> {
        ports
            .iter()
            .map(|p| {
                if Self::is_port_free(&p.port_name) {
                    PortState::Free
                } else {
                    PortState::OccupiedByOther
                }
            })
            .collect()
    }

    /// Toggle the selected port's occupancy by this app. No-op if other program occupies the port.
    pub fn toggle_selected_port(&mut self) {
        // Throttle rapid toggles to prevent OS / driver self-lock due to fast open/close bursts.
        let now = std::time::Instant::now();
        if let Some(last) = self.last_port_toggle {
            if now.duration_since(last).as_millis() < self.port_toggle_min_interval_ms as u128 {
                // Provide user feedback (localized)
                self.set_error(crate::i18n::lang().protocol.toggle_too_fast.clone());
                return; // Ignore rapid toggle
            }
        }
        self.busy = true;
        if self.ports.is_empty() {
            self.busy = false;
            return;
        }
        let i = self.selected;
        // If selected is beyond real ports, handle virtual items
        let special_base = self.ports.len();
        if i >= special_base {
            let rel = i - special_base;
            if rel == 0 {
                // Refresh
                self.refresh();
            } else {
                // Manual specify: not implemented here; set an info / error
                self.set_error(
                    "Manual device specify: only supported on Linux and not implemented yet",
                );
            }
            return;
        }
        if let Some(state) = self.port_states.get_mut(i) {
            match state {
                PortState::Free => {
                    // Try to open and hold the port
                    let port_name = self.ports[i].port_name.clone();
                    match sp_new(&port_name, 9600)
                        .timeout(Duration::from_millis(200))
                        .open()
                    {
                        Ok(handle) => {
                            // No longer store the raw handle separately; pass it directly into the runtime wrapper
                            if let Some(hslot) = self.port_handles.get_mut(i) {
                                *hslot = None;
                            }
                            *state = PortState::OccupiedByThis;
                            // Start runtime listener thread
                            let cfg = self.current_serial_config().unwrap_or_default();
                            if let Ok(rt) = PortRuntimeHandle::from_existing(handle, cfg.clone()) {
                                if let Some(rslot) = self.port_runtimes.get_mut(i) {
                                    *rslot = Some(rt);
                                }
                            } else {
                                self.set_error(format!("failed to spawn runtime for {port_name}"));
                            }
                            // Record successful state change for throttle
                            self.last_port_toggle = Some(now);
                        }
                        Err(e) => {
                            // Cannot open -> likely occupied by other
                            *state = PortState::OccupiedByOther;
                            self.set_error(format!("failed to open {}: {}", port_name, e));
                        }
                    }
                }
                PortState::OccupiedByThis => {
                    // Drop handle
                    if let Some(hslot) = self.port_handles.get_mut(i) {
                        *hslot = None;
                    }
                    if let Some(rslot) = self.port_runtimes.get_mut(i) {
                        if let Some(rt) = rslot.take() {
                            let _ = rt.cmd_tx.send(RuntimeCommand::Stop);
                        }
                    }
                    *state = PortState::Free;
                    // Record successful state change for throttle
                    self.last_port_toggle = Some(now);
                }
                PortState::OccupiedByOther => {
                    // Don't change
                }
            }
        }
    }

    /// Build SerialConfig from current subpage form (if any)
    pub fn current_serial_config(&self) -> Option<SerialConfig> {
        let form = self.subpage_form.as_ref()?;
        Some(SerialConfig {
            baud: form.baud,
            data_bits: form.data_bits,
            stop_bits: form.stop_bits,
            parity: form.parity,
        })
    }

    /// Hot-sync runtime config with UI form
    pub fn sync_runtime_configs(&mut self) {
        if self.selected >= self.ports.len() {
            return;
        }
        if let Some(Some(rt)) = self.port_runtimes.get(self.selected) {
            if let Some(new_cfg) = self.current_serial_config() {
                if new_cfg != rt.current_cfg {
                    let _ = rt.cmd_tx.send(RuntimeCommand::Reconfigure(new_cfg.clone()));
                    if let Some(rtm) = self
                        .port_runtimes
                        .get_mut(self.selected)
                        .and_then(|o| o.as_mut())
                    {
                        rtm.current_cfg = new_cfg;
                    }
                }
            }
        }
        self.busy = false;
    }

    /// Called by core loop to advance spinner frame (UI reads spinner_frame when busy)
    pub fn tick_spinner(&mut self) {
        if self.busy {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
    }

    /// Drain runtime events and push to logs / errors
    pub fn drain_runtime_events(&mut self) {
        // Collect log additions first to avoid nested mutable borrows during iteration
        let mut pending_logs: Vec<LogEntry> = Vec::new();
        let mut pending_error: Option<String> = None;
        // queued (runtime index, response frame) to send after borrow scope
        let mut queued_responses: Vec<(usize, Vec<u8>)> = Vec::new();
        let selected = self.selected;
        for (idx, opt_rt) in self.port_runtimes.iter_mut().enumerate() {
            if let Some(rt) = opt_rt.as_mut() {
                while let Ok(evt) = rt.evt_rx.try_recv() {
                    match evt {
                        RuntimeEvent::FrameReceived(bytes) => {
                            if idx == selected {
                                // First attempt to interpret frame as a response (most common in SlaveStack polling mode)
                                let mut handled = false;
                                if self.port_mode == PortMode::SlaveStack {
                                    if let Some((sid, func, data)) =
                                        parse_modbus_response(bytes.as_ref())
                                    {
                                        let dbg_resp = ModbusResponse {
                                            slave_id: sid,
                                            function: func,
                                            data: data.clone(),
                                        };
                                        if let Some(form) = self.subpage_form.as_mut() {
                                            // Determine register mode from function code
                                            let mode = match func {
                                                0x01 => Some(RegisterMode::Coils),
                                                0x02 => Some(RegisterMode::DiscreteInputs),
                                                0x03 => Some(RegisterMode::Holding),
                                                0x04 => Some(RegisterMode::Input),
                                                _ => None,
                                            };
                                            if let Some(m) = mode {
                                                for reg in form.registers.iter_mut() {
                                                    if reg.slave_id != sid || reg.mode != m {
                                                        continue;
                                                    }
                                                    // Expected data size check
                                                    let expected = match m {
                                                        RegisterMode::Coils
                                                        | RegisterMode::DiscreteInputs => {
                                                            (reg.length as usize + 7) / 8
                                                        }
                                                        RegisterMode::Holding
                                                        | RegisterMode::Input => {
                                                            reg.length as usize * 2
                                                        }
                                                    };
                                                    if data.len() != expected {
                                                        continue;
                                                    }
                                                    // Update values vector
                                                    match m {
                                                        RegisterMode::Coils
                                                        | RegisterMode::DiscreteInputs => {
                                                            // Bit unpack (high bit -> low bit each byte)
                                                            let mut bits: Vec<u8> =
                                                                Vec::with_capacity(
                                                                    reg.length as usize,
                                                                );
                                                            for b in &data {
                                                                for i in (0..8).rev() {
                                                                    if bits.len()
                                                                        >= reg.length as usize
                                                                    {
                                                                        break;
                                                                    }
                                                                    bits.push(
                                                                        if (b & (1 << i)) != 0 {
                                                                            1
                                                                        } else {
                                                                            0
                                                                        },
                                                                    );
                                                                }
                                                            }
                                                            reg.values = bits;
                                                        }
                                                        RegisterMode::Holding
                                                        | RegisterMode::Input => {
                                                            // Interpret each register (two bytes BE); store low byte for UI (FIXME: support 16‑bit display later)
                                                            let mut vals: Vec<u8> =
                                                                Vec::with_capacity(
                                                                    reg.length as usize,
                                                                );
                                                            for chunk in data.chunks_exact(2) {
                                                                if vals.len() == reg.length as usize
                                                                {
                                                                    break;
                                                                }
                                                                vals.push(chunk[1]);
                                                            }
                                                            reg.values = vals;
                                                        }
                                                    }
                                                    reg.req_success =
                                                        reg.req_success.saturating_add(1);
                                                }
                                            }
                                        }
                                        // Push a parsed debug log entry
                                        pending_logs.push(LogEntry {
                                            when: Local::now(),
                                            raw: format!("{:?}", dbg_resp),
                                            parsed: None,
                                        });
                                        handled = true;
                                    }
                                }
                                // Master mode acts as simulated slave: auto respond to recognized requests
                                if !handled && self.port_mode == PortMode::Master {
                                    let frame = bytes.as_ref();
                                    if frame.len() >= 8 {
                                        // minimal RTU frame length
                                        let sid = frame[0];
                                        let func = frame[1];
                                        let mut reply: Option<Vec<u8>> = None;
                                        // Snapshot of registers for read operations to avoid holding mutable borrow
                                        let regs_snapshot: Option<Vec<RegisterEntry>> =
                                            self.subpage_form.as_ref().map(|f| f.registers.clone());
                                        let find_coil_bit =
                                            |addr: u16, regs: &Vec<RegisterEntry>| -> u8 {
                                                for reg in regs {
                                                    if reg.slave_id == sid
                                                        && (reg.mode == RegisterMode::Coils
                                                            || reg.mode
                                                                == RegisterMode::DiscreteInputs)
                                                    {
                                                        let start = reg.address;
                                                        let end = start + reg.length - 1;
                                                        if addr >= start && addr <= end {
                                                            let off = (addr - start) as usize;
                                                            return *reg
                                                                .values
                                                                .get(off)
                                                                .unwrap_or(&0);
                                                        }
                                                    }
                                                }
                                                0
                                            };
                                        let find_holding =
                                            |addr: u16, regs: &Vec<RegisterEntry>| -> u8 {
                                                for reg in regs {
                                                    if reg.slave_id == sid
                                                        && (reg.mode == RegisterMode::Holding
                                                            || reg.mode == RegisterMode::Input)
                                                    {
                                                        let start = reg.address;
                                                        let end = start + reg.length - 1;
                                                        if addr >= start && addr <= end {
                                                            let off = (addr - start) as usize;
                                                            return *reg
                                                                .values
                                                                .get(off)
                                                                .unwrap_or(&0);
                                                        }
                                                    }
                                                }
                                                0
                                            };
                                        match func {
                                            0x01 | 0x02 | 0x03 | 0x04 => {
                                                // read functions
                                                // parse start & qty
                                                let start =
                                                    u16::from_be_bytes([frame[2], frame[3]]);
                                                let qty =
                                                    u16::from_be_bytes([frame[4], frame[5]]).max(1);
                                                let mut data: Vec<u8> = Vec::new();
                                                if func == 0x01 || func == 0x02 {
                                                    // coils / discretes bits
                                                    let mut bit_acc: Vec<u8> =
                                                        Vec::with_capacity(qty as usize);
                                                    if let Some(regs) = regs_snapshot.as_ref() {
                                                        for i in 0..qty {
                                                            bit_acc.push(find_coil_bit(
                                                                start + i,
                                                                regs,
                                                            ));
                                                        }
                                                    } else {
                                                        bit_acc.resize(qty as usize, 0);
                                                    }
                                                    // pack LSB-first per Modbus spec
                                                    let mut byte: u8 = 0;
                                                    let mut count = 0;
                                                    for (i, b) in bit_acc.iter().enumerate() {
                                                        if *b != 0 {
                                                            byte |= 1 << (i % 8);
                                                        }
                                                        count += 1;
                                                        if count == 8 {
                                                            data.push(byte);
                                                            byte = 0;
                                                            count = 0;
                                                        }
                                                    }
                                                    if count > 0 {
                                                        data.push(byte);
                                                    }
                                                } else {
                                                    // holdings / input registers
                                                    if let Some(regs) = regs_snapshot.as_ref() {
                                                        for i in 0..qty {
                                                            let v = find_holding(start + i, regs);
                                                            data.push(0);
                                                            data.push(v);
                                                        }
                                                    } else {
                                                        for _ in 0..qty {
                                                            data.push(0);
                                                            data.push(0);
                                                        }
                                                    }
                                                }
                                                let mut resp = Vec::with_capacity(5 + data.len());
                                                resp.push(sid);
                                                resp.push(func);
                                                resp.push(data.len() as u8);
                                                resp.extend_from_slice(&data);
                                                let crc = modbus_crc16(&resp);
                                                resp.push((crc & 0xFF) as u8);
                                                resp.push((crc >> 8) as u8);
                                                reply = Some(resp);
                                            }
                                            0x05 => {
                                                // write single coil
                                                let addr = u16::from_be_bytes([frame[2], frame[3]]);
                                                let value_hi = frame[4];
                                                let value_lo = frame[5];
                                                let on = value_hi == 0xFF && value_lo == 0x00;
                                                if let Some(form) = self.subpage_form.as_mut() {
                                                    for reg in form.registers.iter_mut() {
                                                        if reg.slave_id == sid
                                                            && (reg.mode == RegisterMode::Coils
                                                                || reg.mode
                                                                    == RegisterMode::DiscreteInputs)
                                                        {
                                                            let start = reg.address;
                                                            let end = start + reg.length - 1;
                                                            if addr >= start && addr <= end {
                                                                let off = (addr - start) as usize;
                                                                if off < reg.values.len() {
                                                                    reg.values[off] =
                                                                        if on { 1 } else { 0 };
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                // echo request as response
                                                reply = Some(frame[..8].to_vec());
                                            }
                                            0x06 => {
                                                // write single holding
                                                let addr = u16::from_be_bytes([frame[2], frame[3]]);
                                                let val_lo = frame[5]; // store low byte
                                                if let Some(form) = self.subpage_form.as_mut() {
                                                    for reg in form.registers.iter_mut() {
                                                        if reg.slave_id == sid
                                                            && (reg.mode == RegisterMode::Holding
                                                                || reg.mode == RegisterMode::Input)
                                                        {
                                                            let start = reg.address;
                                                            let end = start + reg.length - 1;
                                                            if addr >= start && addr <= end {
                                                                let off = (addr - start) as usize;
                                                                if off < reg.values.len() {
                                                                    reg.values[off] = val_lo;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                reply = Some(frame[..8].to_vec());
                                            }
                                            0x0F => {
                                                // write multiple coils
                                                if frame.len() >= 9 {
                                                    // addr qty bytecount ... crc
                                                    let addr =
                                                        u16::from_be_bytes([frame[2], frame[3]]);
                                                    let qty =
                                                        u16::from_be_bytes([frame[4], frame[5]]);
                                                    let byte_count = frame[6] as usize;
                                                    if frame.len() >= 7 + byte_count + 2 {
                                                        if let Some(form) =
                                                            self.subpage_form.as_mut()
                                                        {
                                                            for reg in form.registers.iter_mut() {
                                                                if reg.slave_id==sid && (reg.mode==RegisterMode::Coils || reg.mode==RegisterMode::DiscreteInputs) { let start=reg.address; let end=start+reg.length-1; for i in 0..qty { let cur=addr+i; if cur<start || cur> end { continue; } let off=(cur-start) as usize; let byte_index=(i/8) as usize; let bit_index= i %8; if byte_index < byte_count { let b=frame[7+ byte_index]; let bit=(b >> bit_index) & 1; if off < reg.values.len() { reg.values[off]=bit; } } } }
                                                            }
                                                        }
                                                        // response: id func addr qty crc
                                                        let mut resp = Vec::with_capacity(8);
                                                        resp.extend_from_slice(&frame[0..6]);
                                                        let crc = modbus_crc16(&resp);
                                                        resp.push((crc & 0xFF) as u8);
                                                        resp.push((crc >> 8) as u8);
                                                        reply = Some(resp);
                                                    }
                                                }
                                            }
                                            0x10 => {
                                                // write multiple holdings
                                                if frame.len() >= 9 {
                                                    let addr =
                                                        u16::from_be_bytes([frame[2], frame[3]]);
                                                    let qty =
                                                        u16::from_be_bytes([frame[4], frame[5]]);
                                                    let byte_count = frame[6] as usize;
                                                    if frame.len() >= 7 + byte_count + 2 {
                                                        if let Some(form) =
                                                            self.subpage_form.as_mut()
                                                        {
                                                            for i in 0..qty {
                                                                let base = 7 + (i as usize) * 2;
                                                                if base + 1 < frame.len() {
                                                                    let lo = frame[base + 1];
                                                                    for reg in
                                                                        form.registers.iter_mut()
                                                                    {
                                                                        if reg.slave_id==sid && (reg.mode==RegisterMode::Holding || reg.mode==RegisterMode::Input) { let start=reg.address; let end=start+reg.length-1; let cur=addr+i; if cur>=start && cur<=end { let off=(cur-start) as usize; if off < reg.values.len() { reg.values[off]=lo; } } }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        let mut resp = Vec::with_capacity(8);
                                                        resp.extend_from_slice(&frame[0..6]);
                                                        let crc = modbus_crc16(&resp);
                                                        resp.push((crc & 0xFF) as u8);
                                                        resp.push((crc >> 8) as u8);
                                                        reply = Some(resp);
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                        if let Some(resp) = reply {
                                            let hex_resp = resp
                                                .iter()
                                                .map(|b| format!("{:02x}", b))
                                                .collect::<Vec<_>>()
                                                .join(" ");
                                            queued_responses.push((idx, resp.clone()));
                                            pending_logs.push(LogEntry {
                                                when: Local::now(),
                                                raw: format!("reply: {hex_resp}"),
                                                parsed: None,
                                            });
                                        }
                                    }
                                }
                                if !handled {
                                    // Fallback: treat as request reaching us (acts as slave) and update counters heuristically
                                    if let Some((slave_id, func, addr, qty)) =
                                        parse_modbus_request(bytes.as_ref())
                                    {
                                        if let Some(form) = self.subpage_form.as_mut() {
                                            let target_modes: &[RegisterMode] = match func {
                                                0x01 => &[RegisterMode::Coils],
                                                0x02 => &[RegisterMode::DiscreteInputs],
                                                0x03 => &[RegisterMode::Holding],
                                                0x04 => &[RegisterMode::Input],
                                                0x05 => &[RegisterMode::Coils],
                                                0x06 => &[RegisterMode::Holding],
                                                0x0F => &[RegisterMode::Coils],
                                                0x10 => &[RegisterMode::Holding],
                                                _ => &[],
                                            };
                                            if !target_modes.is_empty() {
                                                for reg in form.registers.iter_mut() {
                                                    if reg.slave_id != slave_id {
                                                        continue;
                                                    }
                                                    if !target_modes.iter().any(|m| *m == reg.mode)
                                                    {
                                                        continue;
                                                    }
                                                    let reg_start = reg.address as u32;
                                                    let reg_end = reg_start + reg.length as u32 - 1;
                                                    let req_start = addr as u32;
                                                    let req_end = req_start + qty as u32 - 1;
                                                    if req_end < reg_start || req_start > reg_end {
                                                        continue;
                                                    }
                                                    reg.req_total = reg.req_total.saturating_add(1);
                                                    if req_start >= reg_start && req_end <= reg_end
                                                    {
                                                        reg.req_success =
                                                            reg.req_success.saturating_add(1);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                let hex = bytes
                                    .iter()
                                    .map(|b| format!("{:02x}", b))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                pending_logs.push(LogEntry {
                                    when: Local::now(),
                                    raw: hex,
                                    parsed: None,
                                });
                            }
                        }
                        RuntimeEvent::FrameSent(bytes) => {
                            if idx == selected {
                                let hex = bytes
                                    .iter()
                                    .map(|b| format!("{:02x}", b))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                pending_logs.push(LogEntry {
                                    when: Local::now(),
                                    raw: format!("sent: {hex}"),
                                    parsed: None,
                                });
                            }
                        }
                        RuntimeEvent::Reconfigured(cfg) => {
                            if idx == selected {
                                pending_logs.push(LogEntry { when: Local::now(), raw: format!("reconfigured: baud={} data_bits={} stop_bits={} parity={:?}", cfg.baud, cfg.data_bits, cfg.stop_bits, cfg.parity), parsed: None });
                            }
                        }
                        RuntimeEvent::Error(e) => {
                            if idx == selected {
                                pending_error = Some(e);
                            }
                        }
                        RuntimeEvent::Stopped => {}
                    }
                }
            }
        }
        for l in pending_logs {
            self.append_log(l);
        }
        if let Some(e) = pending_error {
            self.set_error(e);
        }
        // Now send queued responses (no conflicting borrows)
        for (i, resp) in queued_responses {
            if let Some(Some(rt)) = self.port_runtimes.get(i) {
                let _ = rt.cmd_tx.send(RuntimeCommand::Write(resp));
            }
        }
    }

    pub fn toggle_auto_refresh(&mut self) {
        self.auto_refresh = !self.auto_refresh;
    }
    pub fn next(&mut self) {
        let total = self.ports.len();
        if total == 0 {
            return;
        }
        self.save_current_port_state();
        self.selected = (self.selected + 1) % total;
        self.load_current_port_state();
    }

    pub fn prev(&mut self) {
        let total = self.ports.len();
        if total == 0 {
            return;
        }
        self.save_current_port_state();
        if self.selected == 0 {
            self.selected = total - 1;
        } else {
            self.selected -= 1;
        }
        self.load_current_port_state();
    }

    /// Navigate among visual rows in the left pane including the two trailing virtual items
    /// (Refresh and Manual specify). This is used by the TUI navigation so the user can
    /// Select those bottom options even though the logical model's next()/prev() operate on
    /// Real ports only for test stability.
    pub fn next_visual(&mut self) {
        let total = self.ports.len().saturating_add(2);
        if total == 0 {
            return;
        }
        let was_real = self.selected < self.ports.len();
        if was_real {
            self.save_current_port_state();
        }
        self.selected = (self.selected + 1) % total;
        if self.selected < self.ports.len() {
            self.load_current_port_state();
        }
    }

    pub fn prev_visual(&mut self) {
        let total = self.ports.len().saturating_add(2);
        if total == 0 {
            return;
        }
        let was_real = self.selected < self.ports.len();
        if was_real {
            self.save_current_port_state();
        }
        if self.selected == 0 {
            self.selected = total - 1;
        } else {
            self.selected -= 1;
        }
        if self.selected < self.ports.len() {
            self.load_current_port_state();
        }
    }

    /// Drive periodic polling for slave listen entries (actively send Modbus queries for read‑type entries when in Master mode).
    /// For now, only generates synthetic increments (placeholder) if no active writer exists.
    pub fn drive_slave_polling(&mut self) {
        // Only when current port is occupied & active subpage in SlaveStack mode
        if self.port_mode != PortMode::SlaveStack {
            return;
        }
        if self.active_subpage != Some(PortMode::SlaveStack) {
            return;
        }
        if self.polling_paused {
            return;
        }
        let now = std::time::Instant::now();
        if let Some(form) = self.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                if now >= reg.next_poll_at {
                    // Build Modbus RTU read request using rmodbus helpers when possible
                    let mut raw: Vec<u8> = Vec::new();
                    let ok_build = (|| {
                        use rmodbus::{client::ModbusRequest, ModbusProto};
                        let mut req = ModbusRequest::new(reg.slave_id, ModbusProto::Rtu);
                        let qty = reg.length.min(125); // adhere to Modbus limits
                        match reg.mode {
                            RegisterMode::Coils => req
                                .generate_get_coils(reg.address, qty, &mut raw)
                                .map(|_| true)
                                .map_err(|_| ()),
                            RegisterMode::Holding => req
                                .generate_get_holdings(reg.address, qty, &mut raw)
                                .map(|_| true)
                                .map_err(|_| ()),
                            // Fallback for unsupported helper usage
                            RegisterMode::DiscreteInputs | RegisterMode::Input => Err(()),
                        }
                    })();
                    if ok_build.is_err() {
                        // Manual build (legacy path) for modes we didn't generate above
                        let func = match reg.mode {
                            RegisterMode::Coils => 0x01,
                            RegisterMode::DiscreteInputs => 0x02,
                            RegisterMode::Holding => 0x03,
                            RegisterMode::Input => 0x04,
                        };
                        let qty = reg.length.min(125);
                        raw.push(reg.slave_id);
                        raw.push(func);
                        raw.push((reg.address >> 8) as u8);
                        raw.push((reg.address & 0xFF) as u8);
                        raw.push((qty >> 8) as u8);
                        raw.push((qty & 0xFF) as u8);
                        let crc = modbus_crc16(&raw);
                        raw.push((crc & 0xFF) as u8); // low byte first
                        raw.push((crc >> 8) as u8);
                    }
                    if !raw.is_empty() && self.selected < self.port_runtimes.len() {
                        if let Some(Some(rt)) = self.port_runtimes.get(self.selected) {
                            let _ = rt
                                .cmd_tx
                                .send(crate::protocol::runtime::RuntimeCommand::Write(raw));
                            reg.req_total = reg.req_total.saturating_add(1);
                        }
                    }
                    reg.next_poll_at =
                        now + std::time::Duration::from_millis(reg.refresh_ms as u64);
                }
            }
        }
    }
}

/// Compute Modbus RTU CRC16 (little-endian in frame: low byte then high byte)
fn modbus_crc16(data: &[u8]) -> u16 {
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

/// Minimal Modbus RTU request parser (function subset) returning (slave_id, function, start_addr, quantity).
/// Expects at least 8 bytes (id func addr_hi addr_lo qty_hi qty_lo crc_lo crc_hi).
/// Returns None if frame is too short or CRC check fails (CRC not verified here yet) or function unsupported for counting.
fn parse_modbus_request(frame: &[u8]) -> Option<(u8, u8, u16, u16)> {
    if frame.len() < 8 {
        return None;
    }
    let slave_id = frame[0];
    let func = frame[1];
    match func {
        0x01 | 0x02 | 0x03 | 0x04 => {
            if frame.len() < 8 {
                return None;
            }
            let addr = u16::from_be_bytes([frame[2], frame[3]]);
            let qty = u16::from_be_bytes([frame[4], frame[5]]);
            Some((slave_id, func, addr, qty.max(1)))
        }
        0x05 | 0x06 => {
            // single coil/register write
            if frame.len() < 8 {
                return None;
            }
            let addr = u16::from_be_bytes([frame[2], frame[3]]);
            Some((slave_id, func, addr, 1))
        }
        0x0F | 0x10 => {
            // multiple write; quantity at bytes 4..6
            if frame.len() < 9 {
                return None;
            }
            let addr = u16::from_be_bytes([frame[2], frame[3]]);
            let qty = u16::from_be_bytes([frame[4], frame[5]]);
            Some((slave_id, func, addr, qty.max(1)))
        }
        _ => None,
    }
}

/// Attempt to parse a Modbus RTU response (read functions 0x01-0x04) and return (id, func, data_bytes)
fn parse_modbus_response(frame: &[u8]) -> Option<(u8, u8, Vec<u8>)> {
    if frame.len() < 5 {
        // id func byte_count ... crc
        return None;
    }
    let slave_id = frame[0];
    let func = frame[1];
    if !(1..=4).contains(&func) {
        return None;
    }
    // Requests are always 8 bytes; filter them out early to reduce ambiguity
    if frame.len() == 8 {
        return None;
    }
    // CRC check
    if frame.len() < 5 {
        return None;
    }
    let crc_frame = ((frame[frame.len() - 1] as u16) << 8) | frame[frame.len() - 2] as u16; // low, high
    let calc = modbus_crc16(&frame[..frame.len() - 2]);
    if crc_frame != calc {
        return None;
    }
    let byte_count = frame[2] as usize;
    if frame.len() != byte_count + 5 {
        return None;
    } // id func byte_count data... crc_lo crc_hi
    if byte_count == 0 {
        return None;
    }
    let data = frame[3..3 + byte_count].to_vec();
    Some((slave_id, func, data))
}

/// High-level decoded Modbus response for logging.
#[derive(Clone)]
struct ModbusResponse {
    slave_id: u8,
    function: u8,
    /// Raw data payload bytes (already CRC‑stripped)
    data: Vec<u8>,
}

impl std::fmt::Debug for ModbusResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let func_name = match self.function {
            0x01 => "Read Coils",
            0x02 => "Read Discrete Inputs",
            0x03 => "Read Holding Registers",
            0x04 => "Read Input Registers",
            _ => "Unknown",
        };
        writeln!(
            f,
            "ModbusResponse {{ id: {}, func: 0x{:02X} ({func_name}), bytes: {} }}",
            self.slave_id,
            self.function,
            self.data.len()
        )?;
        match self.function {
            0x01 | 0x02 => {
                let mut bits: Vec<bool> = Vec::new();
                for b in &self.data {
                    for i in (0..8).rev() {
                        bits.push((b & (1 << i)) != 0);
                    }
                }
                writeln!(f, "  coils/discretes(bits={}): {:?}", bits.len(), bits)?;
            }
            0x03 | 0x04 => {
                let regs = self
                    .data
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>();
                writeln!(f, "  registers(count={}): {:?}", regs.len(), regs)?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl Status {
    /// Reset counters & logs and pause polling while user edits slave parameters.
    pub fn pause_and_reset_slave_listen(&mut self) {
        if self.port_mode != PortMode::SlaveStack {
            return;
        }
        if let Some(form) = self.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                reg.req_success = 0;
                reg.req_total = 0;
                for v in reg.values.iter_mut() {
                    *v = 0;
                }
                // Push next poll far away until resume
                reg.next_poll_at = std::time::Instant::now() + std::time::Duration::from_secs(3600);
            }
        }
        self.logs.clear();
        self.log_selected = 0;
        self.log_view_offset = 0;
        self.polling_paused = true;
    }

    /// Resume polling immediately after parameters confirmed.
    pub fn resume_slave_listen(&mut self) {
        if self.port_mode != PortMode::SlaveStack {
            return;
        }
        if let Some(form) = self.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                reg.next_poll_at = std::time::Instant::now();
            }
        }
        self.polling_paused = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{SerialPortInfo, SerialPortType};

    fn fake_port(name: &str) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.to_string(),
            port_type: SerialPortType::Unknown,
        }
    }

    #[test]
    fn test_navigation() {
        let ports = vec![fake_port("COM1"), fake_port("COM2")];
        let mut app = Status::with_ports(ports);
        assert_eq!(app.selected, 0);
        app.next();
        assert_eq!(app.selected, 1);
        app.next();
        assert_eq!(app.selected, 0);
        app.prev();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn test_focus_and_refresh() {
        let ports = vec![fake_port("COM1")];
        let mut app = Status::with_ports(ports);
        // Call refresh (may change ports depending on environment)
        app.refresh();
        // Ensure selected is in bounds
        if !app.ports.is_empty() {
            assert!(app.selected < app.ports.len());
        }
    }

    #[test]
    fn test_log_paging_and_adjust() {
        let mut s = Status::with_ports(vec![]);
        // Fill 20 log entries.
        for i in 0..20 {
            s.append_log(LogEntry {
                when: Local::now(),
                raw: format!("payload{i}"),
                parsed: None,
            });
        }
        // Initial auto_scroll should be anchored at the end.
        assert_eq!(s.log_view_offset, 19);
        s.page_up(5);
        assert!(s.log_view_offset <= 19);
        let prev = s.log_view_offset;
        s.page_down(5);
        assert!(s.log_view_offset >= prev);
        // Select the first entry; adjusting the view should move the offset upward (no longer stuck to bottom).
        s.log_selected = 0;
        s.adjust_log_view(40); // Simulated terminal height.
        assert!(s.log_view_offset <= 19);
    }
}
