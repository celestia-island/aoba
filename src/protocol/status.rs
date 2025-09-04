//! Runtime status & ModBus unified data structures.
use chrono::{DateTime, Local};
use std::{
    cmp::{max, min},
    collections::HashMap,
    time::Duration,
    vec::Vec,
};

use serialport::{SerialPort, SerialPortInfo};

use crate::protocol::runtime::{PortRuntimeHandle, RuntimeCommand, RuntimeEvent, SerialConfig};
use crate::{
    i18n::lang,
    protocol::modbus::{
        generate_pull_get_coils_request, generate_pull_get_discrete_inputs_request,
        generate_pull_get_holdings_request, generate_pull_get_inputs_request, parse_pull_get_coils,
        parse_pull_get_discrete_inputs, parse_pull_get_holdings, parse_pull_get_inputs,
    },
};

// --- Reintroduced data structures lost during previous merge/conflict operations ---
// Keep these aligned with usages in TUI modules (pages/modbus.rs, utils/edit.rs, tui/mod.rs)

#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub origin: String,
    pub rw: String,      // "R" or "W"
    pub command: String, // function name or descriptor
    pub slave_id: u8,
    pub address: u16,
    pub length: u16,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub when: DateTime<Local>,
    pub raw: String,
    pub parsed: Option<ParsedRequest>,
}

#[derive(Debug, Clone)]
pub struct SubpageForm {
    // Serial config editing (original config panel expectations)
    pub editing: bool,
    pub loop_enabled: bool,
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: serialport::Parity,
    pub cursor: usize, // which config/register line currently highlighted
    pub editing_field: Option<EditingField>,
    pub input_buffer: String,             // generic edit buffer
    pub edit_choice_index: Option<usize>, // selector index when cycling preset options
    pub edit_confirmed: bool,             // custom value entered confirmed stage

    // Unified master/slave register list
    pub registers: Vec<RegisterEntry>,
    pub master_cursor: usize,
    pub master_field_selected: bool,
    pub master_field_editing: bool,
    pub master_edit_field: Option<MasterEditField>,
    pub master_edit_index: Option<usize>,
    pub master_input_buffer: String,
}

impl Default for SubpageForm {
    fn default() -> Self {
        Self {
            editing: false,
            loop_enabled: false,
            baud: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: serialport::Parity::None,
            cursor: 0,
            editing_field: None,
            input_buffer: String::new(),
            edit_choice_index: None,
            edit_confirmed: false,
            registers: Vec::new(),
            master_cursor: 0,
            master_field_selected: false,
            master_field_editing: false,
            master_edit_field: None,
            master_edit_index: None,
            master_input_buffer: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerPortState {
    pub subpage_active: bool,
    pub subpage_form: Option<SubpageForm>,
    pub subpage_tab_index: usize,
    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
    pub app_mode: AppMode,
}

// If LOG_GROUP_HEIGHT was defined previously in tui module, re‑import; else define fallback.
#[allow(dead_code)]
const LOG_GROUP_HEIGHT: usize = 3; // fallback; adjust if original constant differs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRole {
    Master,
    Slave,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterMode {
    Coils = 1,
    DiscreteInputs = 2,
    Holding = 3,
    Input = 4,
}
impl RegisterMode {
    pub const fn all() -> &'static [RegisterMode] {
        &[
            RegisterMode::Coils,
            RegisterMode::DiscreteInputs,
            RegisterMode::Holding,
            RegisterMode::Input,
        ]
    }
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => Self::Coils,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterEntry {
    pub role: EntryRole, // Master or Slave role per entry
    pub slave_id: u8,
    pub mode: RegisterMode,
    pub address: u16,     // start address
    pub length: u16,      // number of points / registers
    pub values: Vec<u16>, // value items per address: coils/discrete store 0/1, holding/input store full 16-bit register
    pub refresh_ms: u32,  // polling interval
    pub next_poll_at: std::time::Instant,
    pub req_success: u32,
    pub req_total: u32,
    // Pending Modbus read requests waiting for a matching response
    pub pending_requests: Vec<PendingRequest>,
}

impl Default for RegisterEntry {
    fn default() -> Self {
        Self {
            role: EntryRole::Slave,
            slave_id: 1,
            mode: RegisterMode::Holding,
            address: 0,
            length: 1,
            values: vec![0u16],
            refresh_ms: 1000,
            next_poll_at: std::time::Instant::now(),
            req_success: 0,
            req_total: 0,
            pending_requests: Vec::new(),
        }
    }
}

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct PendingRequest {
    func: u8,
    address: u16,
    count: u16,
    sent_at: std::time::Instant,
    request: Arc<Mutex<rmodbus::client::ModbusRequest>>,
}

// Keep the original ModbusRequest used to generate the raw frame so we can
// call its parse helpers against the exact request context (avoids mismatches
// caused by recreated requests or different internal state).
impl PendingRequest {
    pub fn new(
        func: u8,
        address: u16,
        count: u16,
        sent_at: std::time::Instant,
        request: rmodbus::client::ModbusRequest,
    ) -> Self {
        Self {
            func,
            address,
            count,
            sent_at,
            request: Arc::new(Mutex::new(request)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Ascii,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Modbus,
    Mqtt,
}
impl AppMode {
    pub fn cycle(self) -> Self {
        match self {
            AppMode::Modbus => AppMode::Mqtt,
            AppMode::Mqtt => AppMode::Modbus,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            AppMode::Modbus => "ModBus RTU",
            AppMode::Mqtt => "MQTT",
        }
    }
}

#[derive(Debug)]
pub struct Status {
    pub ports: Vec<SerialPortInfo>,
    pub port_extras: Vec<crate::protocol::tty::PortExtra>,
    pub port_states: Vec<PortState>,
    pub port_handles: Vec<Option<Box<dyn SerialPort>>>,
    pub port_runtimes: Vec<Option<PortRuntimeHandle>>,
    pub selected: usize,

    pub auto_refresh: bool,
    pub last_refresh: Option<DateTime<Local>>,

    pub error: Option<(String, DateTime<Local>)>,

    pub subpage_active: bool,
    pub subpage_form: Option<SubpageForm>,
    pub subpage_tab_index: usize,

    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,

    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,

    pub app_mode: AppMode,
    pub mode_overlay_active: bool,
    pub mode_overlay_index: usize,

    pub(crate) per_port_states: HashMap<String, PerPortState>,
    pub last_scan_info: Vec<String>,
    pub last_scan_time: Option<DateTime<Local>>,

    pub busy: bool,
    pub spinner_frame: u8,
    pub polling_paused: bool,
    pub last_port_toggle: Option<std::time::Instant>,
    pub port_toggle_min_interval_ms: u64,
}

impl Status {
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            port_extras: Vec::new(),
            port_states: Vec::new(),
            port_handles: Vec::new(),
            port_runtimes: Vec::new(),
            selected: 0,
            auto_refresh: true,
            last_refresh: None,
            error: None,
            subpage_active: false,
            subpage_form: None,
            subpage_tab_index: 0,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            input_mode: InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
            app_mode: AppMode::Modbus,
            mode_overlay_active: false,
            mode_overlay_index: 0,
            per_port_states: HashMap::new(),
            last_scan_info: Vec::new(),
            last_scan_time: None,
            busy: false,
            spinner_frame: 0,
            polling_paused: false,
            last_port_toggle: None,
            port_toggle_min_interval_ms: PORT_TOGGLE_MIN_INTERVAL_MS,
        }
    }

    pub fn with_ports(ports: Vec<SerialPortInfo>) -> Self {
        let mut s = Status::new();
        s.ports = ports.clone();
        s.port_states = Self::detect_port_states(&s.ports);
        s.port_handles = s.ports.iter().map(|_| None).collect();
        s.port_runtimes = s.ports.iter().map(|_| None).collect();
        s
    }

    /// Final authoritative drain_runtime_events implementation (re-added after cleanup)
    pub fn drain_runtime_events(&mut self) {
        if self.ports.is_empty() {
            return;
        }
        let selected = self.selected;
        let mut pending_logs: Vec<LogEntry> = Vec::new();
        let mut pending_error: Option<String> = None;
        for (idx, rt_opt) in self.port_runtimes.iter_mut().enumerate() {
            if let Some(rt) = rt_opt.as_mut() {
                loop {
                    match rt.evt_rx.try_recv() {
                        Ok(evt) => {
                            match evt {
                                RuntimeEvent::FrameReceived(bytes) => {
                                    if idx != selected {
                                        // only show received frames for selected port
                                        continue;
                                    }
                                    let raw_hex = bytes
                                        .iter()
                                        .map(|b| format!("{:02x}", b))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    let mut consumed = false;
                                    let nowi = std::time::Instant::now();
                                    // try to match this frame against pending requests per register
                                    if let Some(form) = self.subpage_form.as_mut() {
                                        for reg in form.registers.iter_mut() {
                                            if reg.role != EntryRole::Slave {
                                                continue;
                                            }
                                            let mut remove_indices: Vec<usize> = Vec::new();
                                            let pending_len = reg.pending_requests.len();
                                            for pi in 0..pending_len {
                                                // quick check: slave id must match
                                                if bytes.first().copied() != Some(reg.slave_id) {
                                                    break;
                                                }
                                                if let Some(pending) = reg.pending_requests.get(pi)
                                                {
                                                    if bytes.get(1).copied() != Some(pending.func) {
                                                        continue;
                                                    }
                                                    let frame_vec = bytes.to_vec();
                                                    if let Ok(mut saved_req) =
                                                        pending.request.lock()
                                                    {
                                                        let parse_ok: Option<Vec<u8>> = match reg
                                                            .mode
                                                        {
                                                            RegisterMode::Coils => {
                                                                match parse_pull_get_coils(
                                                                    &mut *saved_req,
                                                                    frame_vec.clone(),
                                                                    pending.count,
                                                                ) {
                                                                    Ok(vb) => Some(
                                                                        vb.into_iter()
                                                                            .map(|b| {
                                                                                if b {
                                                                                    1
                                                                                } else {
                                                                                    0
                                                                                }
                                                                            })
                                                                            .collect(),
                                                                    ),
                                                                    Err(e) => {
                                                                        pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (coils): {} raw={}", e, raw_hex), parsed: None });
                                                                        remove_indices.push(pi);
                                                                        consumed = true;
                                                                        None
                                                                    }
                                                                }
                                                            }
                                                            RegisterMode::DiscreteInputs => {
                                                                match parse_pull_get_discrete_inputs(
                                                                    &mut *saved_req,
                                                                    frame_vec.clone(),
                                                                    pending.count,
                                                                ) {
                                                                    Ok(vb) => Some(
                                                                        vb.into_iter()
                                                                            .map(|b| {
                                                                                if b {
                                                                                    1
                                                                                } else {
                                                                                    0
                                                                                }
                                                                            })
                                                                            .collect(),
                                                                    ),
                                                                    Err(e) => {
                                                                        pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (discrete): {} raw={}", e, raw_hex), parsed: None });
                                                                        remove_indices.push(pi);
                                                                        consumed = true;
                                                                        None
                                                                    }
                                                                }
                                                            }
                                                            RegisterMode::Holding => {
                                                                match parse_pull_get_holdings(
                                                                    &mut *saved_req,
                                                                    frame_vec.clone(),
                                                                ) {
                                                                    Ok(v) => Some(
                                                                        v.into_iter()
                                                                            .flat_map(|w| {
                                                                                w.to_be_bytes()
                                                                            })
                                                                            .collect(),
                                                                    ),
                                                                    Err(e) => {
                                                                        pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (holding): {} raw={}", e, raw_hex), parsed: None });
                                                                        remove_indices.push(pi);
                                                                        consumed = true;
                                                                        None
                                                                    }
                                                                }
                                                            }
                                                            RegisterMode::Input => {
                                                                match parse_pull_get_inputs(
                                                                    &mut *saved_req,
                                                                    frame_vec.clone(),
                                                                ) {
                                                                    Ok(v) => Some(
                                                                        v.into_iter()
                                                                            .flat_map(|w| {
                                                                                w.to_be_bytes()
                                                                            })
                                                                            .collect(),
                                                                    ),
                                                                    Err(e) => {
                                                                        pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (input): {} raw={}", e, raw_hex), parsed: None });
                                                                        remove_indices.push(pi);
                                                                        consumed = true;
                                                                        None
                                                                    }
                                                                }
                                                            }
                                                        };
                                                        if let Some(mut bts) = parse_ok {
                                                            // store values: for Holding/Input convert BE pairs into u16; for Coils/Discrete keep 0/1 per address
                                                            match reg.mode {
                                                                RegisterMode::Holding
                                                                | RegisterMode::Input => {
                                                                    if bts.len() % 2 != 0 {
                                                                        bts.push(0);
                                                                    }
                                                                    let regs: Vec<u16> = bts
                                                                        .chunks_exact(2)
                                                                        .map(|c| {
                                                                            u16::from_be_bytes([
                                                                                c[0], c[1],
                                                                            ])
                                                                        })
                                                                        .collect();
                                                                    let mut vals = regs;
                                                                    if vals.len()
                                                                        < reg.length as usize
                                                                    {
                                                                        vals.resize(
                                                                            reg.length as usize,
                                                                            0u16,
                                                                        );
                                                                    }
                                                                    if vals.len()
                                                                        > reg.length as usize
                                                                    {
                                                                        vals.truncate(
                                                                            reg.length as usize,
                                                                        );
                                                                    }
                                                                    reg.values = vals;
                                                                }
                                                                _ => {
                                                                    let mut vals: Vec<u16> = bts
                                                                        .into_iter()
                                                                        .map(|v| v as u16)
                                                                        .collect();
                                                                    if vals.len()
                                                                        < reg.length as usize
                                                                    {
                                                                        vals.resize(
                                                                            reg.length as usize,
                                                                            0u16,
                                                                        );
                                                                    }
                                                                    if vals.len()
                                                                        > reg.length as usize
                                                                    {
                                                                        vals.truncate(
                                                                            reg.length as usize,
                                                                        );
                                                                    }
                                                                    reg.values = vals;
                                                                }
                                                            }
                                                            reg.req_success =
                                                                reg.req_success.saturating_add(1);
                                                            remove_indices.push(pi);
                                                            consumed = true;
                                                            pending_logs.push(LogEntry { when: Local::now(), raw: format!("{} sid={} func=0x{:02X} len={} raw={}", lang().protocol.modbus.log_recv_match, reg.slave_id, pending.func, bytes.len(), raw_hex), parsed: Some(ParsedRequest { origin: "master".to_string(), rw: "R".to_string(), command: format!("func_{:02X}", pending.func), slave_id: reg.slave_id, address: pending.address, length: pending.count }) });
                                                            reg.next_poll_at = nowi
                                                                + std::time::Duration::from_millis(
                                                                    reg.refresh_ms as u64,
                                                                );
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            // remove any marked pending requests (reverse order)
                                            for &ri in remove_indices.iter().rev() {
                                                reg.pending_requests.remove(ri);
                                            }
                                            if consumed {
                                                break;
                                            }
                                        }
                                    }
                                    if !consumed {
                                        let sid = bytes.get(0).copied().unwrap_or(0);
                                        let func = bytes.get(1).copied().unwrap_or(0);
                                        pending_logs.push(LogEntry {
                                            when: Local::now(),
                                            raw: format!(
                                                "{}: {raw_hex}",
                                                lang().protocol.modbus.log_recv_unmatched
                                            ),
                                            parsed: Some(ParsedRequest {
                                                origin: "master".to_string(),
                                                rw: "R".to_string(),
                                                command: format!("func_{:02X}", func),
                                                slave_id: sid,
                                                address: 0,
                                                length: 0,
                                            }),
                                        });
                                        // check for aged-out pending requests and remove them
                                        if let Some(form) = self.subpage_form.as_mut() {
                                            let nowi = std::time::Instant::now();
                                            let timeout_ms = 2000u64;
                                            for reg in form.registers.iter_mut() {
                                                if reg.role != EntryRole::Slave {
                                                    continue;
                                                }
                                                let mut to_remove: Vec<usize> = Vec::new();
                                                for (pi, p) in
                                                    reg.pending_requests.iter().enumerate()
                                                {
                                                    if nowi.duration_since(p.sent_at).as_millis()
                                                        as u64
                                                        > timeout_ms
                                                    {
                                                        pending_logs.push(LogEntry {
                                                            when: Local::now(),
                                                            raw: format!("{} func=0x{:02X} sid={} addr={} cnt={}", lang().protocol.modbus.log_req_timeout, p.func, reg.slave_id, p.address, p.count),
                                                            parsed: Some(ParsedRequest { origin: "master".into(), rw: "R".into(), command: format!("func_{:02X}", p.func), slave_id: reg.slave_id, address: p.address, length: p.count }),
                                                        });
                                                        to_remove.push(pi);
                                                    }
                                                }
                                                if !to_remove.is_empty() {
                                                    reg.next_poll_at = nowi
                                                        + std::time::Duration::from_millis(
                                                            reg.refresh_ms as u64,
                                                        );
                                                    for &ri in to_remove.iter().rev() {
                                                        reg.pending_requests.remove(ri);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                RuntimeEvent::FrameSent(bytes) => {
                                    if idx == selected {
                                        let hex = bytes
                                            .iter()
                                            .map(|b| format!("{:02x}", b))
                                            .collect::<Vec<_>>()
                                            .join(" ");
                                        let sid = bytes.get(0).copied().unwrap_or(0);
                                        let func = bytes.get(1).copied().unwrap_or(0);
                                        let addr = if bytes.len() >= 4 {
                                            u16::from_be_bytes([bytes[2], bytes[3]])
                                        } else {
                                            0
                                        };
                                        let len_or_cnt = if bytes.len() >= 6 {
                                            u16::from_be_bytes([bytes[4], bytes[5]])
                                        } else {
                                            0
                                        };
                                        let cmd = match func {
                                            0x01 => "rd_coils",
                                            0x02 => "rd_discrete",
                                            0x03 => "rd_holdings",
                                            0x04 => "rd_inputs",
                                            0x05 => "wr_coil",
                                            0x06 => "wr_holding",
                                            0x0F => "wr_coils",
                                            0x10 => "wr_holdings",
                                            _ => "func",
                                        };
                                        pending_logs.push(LogEntry {
                                            when: Local::now(),
                                            raw: format!(
                                                "{}: {hex}",
                                                lang().protocol.modbus.log_sent_frame
                                            ),
                                            parsed: Some(ParsedRequest {
                                                origin: "master".to_string(),
                                                rw: "W".to_string(),
                                                command: cmd.to_string(),
                                                slave_id: sid,
                                                address: addr,
                                                length: len_or_cnt,
                                            }),
                                        });
                                    }
                                }
                                RuntimeEvent::Reconfigured(cfg) => {
                                    if idx == selected {
                                        pending_logs.push(LogEntry {
                                            when: Local::now(),
                                            raw: format!(
                                                "{}: baud={} data_bits={} stop_bits={} parity={:?}",
                                                lang().protocol.modbus.log_reconfigured,
                                                cfg.baud,
                                                cfg.data_bits,
                                                cfg.stop_bits,
                                                cfg.parity
                                            ),
                                            parsed: None,
                                        });
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
                        Err(_) => break,
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
    }
    /// Adjust the log view window according to the current terminal height so the selected entry stays visible.
    pub fn adjust_log_view(&mut self, term_height: u16) {
        if self.logs.is_empty() {
            return;
        }
        let bottom_len = if self.error.is_some() || self.subpage_active {
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
        let max_bottom = self.logs.len().saturating_sub(1);
        let new_bottom = (self.log_view_offset).saturating_add(page);
        self.log_view_offset = std::cmp::min(max_bottom, new_bottom);
        // If we've reached the end, re‑enable auto scroll, else freeze.
        if self.log_view_offset >= max_bottom {
            self.log_auto_scroll = true;
        } else {
            self.log_auto_scroll = false;
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
        // Use platform-dispatched enumeration (tty module) instead of local duplicate
        let enriched = crate::protocol::tty::available_ports_enriched();
        let new_ports: Vec<_> = enriched.iter().map(|(p, _)| p.clone()).collect();
        let new_extras: Vec<_> = enriched.into_iter().map(|(_, e)| e).collect();
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
        self.port_extras = new_extras;
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
        let enriched = crate::protocol::tty::available_ports_enriched();
        let new_ports: Vec<_> = enriched.iter().map(|(p, _)| p.clone()).collect();
        let new_extras: Vec<_> = enriched.into_iter().map(|(_, e)| e).collect();
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
        self.port_extras = new_extras;
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

    /// Quick device scan: update last_scan_info / time without re-enumerating serial ports
    pub fn quick_scan(&mut self) {
        self.perform_device_scan();
    }

    pub fn save_current_port_state(&mut self) {
        if self.selected < self.ports.len() {
            if let Some(info) = self.ports.get(self.selected) {
                let snap = PerPortState {
                    subpage_active: self.subpage_active,
                    subpage_form: self.subpage_form.clone(),
                    subpage_tab_index: self.subpage_tab_index,
                    logs: self.logs.clone(),
                    log_selected: self.log_selected,
                    log_view_offset: self.log_view_offset,
                    log_auto_scroll: self.log_auto_scroll,
                    input_mode: self.input_mode,
                    input_editing: self.input_editing,
                    input_buffer: self.input_buffer.clone(),
                    app_mode: self.app_mode,
                };
                self.per_port_states.insert(info.port_name.clone(), snap);
            }
        }
    }

    fn load_current_port_state(&mut self) {
        if self.selected < self.ports.len() {
            if let Some(info) = self.ports.get(self.selected) {
                if let Some(snap) = self.per_port_states.get(&info.port_name).cloned() {
                    self.subpage_active = snap.subpage_active;
                    self.subpage_form = snap.subpage_form;
                    self.subpage_tab_index = snap.subpage_tab_index;
                    self.logs = snap.logs;
                    self.log_selected = snap.log_selected;
                    self.log_view_offset = snap.log_view_offset;
                    self.log_auto_scroll = snap.log_auto_scroll;
                    self.input_mode = snap.input_mode;
                    self.input_editing = snap.input_editing;
                    self.input_buffer = snap.input_buffer;
                    self.app_mode = snap.app_mode;
                } else {
                    // Fresh defaults
                    self.subpage_active = false;
                    self.subpage_form = None;
                    self.subpage_tab_index = 0;
                    self.logs.clear();
                    self.log_selected = 0;
                    self.log_view_offset = 0;
                    self.log_auto_scroll = true;
                    self.input_mode = InputMode::Ascii;
                    self.input_editing = false;
                    self.input_buffer.clear();
                    self.app_mode = AppMode::Modbus;
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
        // Throttle rapid toggles to prevent OS / driver self-lock due to fast open / close bursts.
        let now = std::time::Instant::now();
        if let Some(last) = self.last_port_toggle {
            if now.duration_since(last).as_millis() < self.port_toggle_min_interval_ms as u128 {
                // Provide user feedback (localized)
                self.set_error(lang().protocol.common.toggle_too_fast.clone());
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

    // (legacy duplicate drain_runtime_events block removed)

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
        // Unified mode: poll whenever subpage active and not paused
        if !self.subpage_active {
            return;
        }
        if self.polling_paused {
            return;
        }
        // Respect UI toggle: if user disabled the loop in the subpage form, skip polling
        if self
            .subpage_form
            .as_ref()
            .map(|f| !f.loop_enabled)
            .unwrap_or(true)
        {
            return;
        }
        let now = std::time::Instant::now();
        if let Some(form) = self.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                // Only poll entries that represent a remote Slave device (UI shows 'Slave' for remote)
                if reg.role != EntryRole::Slave {
                    continue;
                }
                // Remove timed-out pending requests so new poll can proceed
                let timeout_ms = 1500u64;
                let mut to_remove: Vec<usize> = Vec::new();
                for (i, p) in reg.pending_requests.iter().enumerate() {
                    if now.duration_since(p.sent_at).as_millis() as u64 > timeout_ms {
                        to_remove.push(i);
                    }
                }
                for &ri in to_remove.iter().rev() {
                    reg.pending_requests.remove(ri);
                }
                if now >= reg.next_poll_at && reg.pending_requests.is_empty() {
                    let qty = reg.length.min(125);
                    let gen = match reg.mode {
                        RegisterMode::Coils => {
                            generate_pull_get_coils_request(reg.slave_id, reg.address, qty)
                        }
                        RegisterMode::DiscreteInputs => generate_pull_get_discrete_inputs_request(
                            reg.slave_id,
                            reg.address,
                            qty,
                        ),
                        RegisterMode::Holding => {
                            generate_pull_get_holdings_request(reg.slave_id, reg.address, qty)
                        }
                        RegisterMode::Input => {
                            generate_pull_get_inputs_request(reg.slave_id, reg.address, qty)
                        }
                    };
                    if let Ok((req_obj, raw)) = gen {
                        if self.selected < self.port_runtimes.len() {
                            if let Some(Some(rt)) = self.port_runtimes.get(self.selected) {
                                if rt.cmd_tx.send(RuntimeCommand::Write(raw)).is_ok() {
                                    reg.req_total = reg.req_total.saturating_add(1);
                                    reg.pending_requests.push(PendingRequest::new(
                                        match reg.mode {
                                            RegisterMode::Coils => 0x01,
                                            RegisterMode::DiscreteInputs => 0x02,
                                            RegisterMode::Holding => 0x03,
                                            RegisterMode::Input => 0x04,
                                        },
                                        reg.address,
                                        qty,
                                        now,
                                        req_obj,
                                    ));
                                }
                            }
                        }
                    }
                    reg.next_poll_at =
                        now + std::time::Duration::from_millis(reg.refresh_ms as u64);
                }
            }
        }
    }

    /// Reset counters & logs and pause polling while user edits slave parameters.
    pub fn pause_and_reset_slave_listen(&mut self) {
        if let Some(form) = self.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                reg.req_success = 0;
                reg.req_total = 0;
                // Clear any pending requests so that resume can send new polls immediately
                reg.pending_requests.clear();
                for v in reg.values.iter_mut() {
                    *v = 0;
                }
                reg.next_poll_at = std::time::Instant::now() + std::time::Duration::from_secs(3600);
            }
        }
        self.polling_paused = true;
    }

    /// Resume polling immediately after parameters confirmed.
    pub fn resume_slave_listen(&mut self) {
        let mut found_master = false;
        // When resuming from pause, clear previous logs so the user sees fresh activity
        self.logs.clear();
        self.log_selected = 0;
        self.log_view_offset = 0;
        if let Some(form) = self.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                reg.next_poll_at = std::time::Instant::now();
                if reg.role == EntryRole::Master {
                    found_master = true;
                }
            }
        }
        self.polling_paused = false;
        // If no master entries exist, add a helpful log entry so user knows why no frames are sent
        if !found_master {
            self.append_log(LogEntry {
                when: Local::now(),
                raw: format!("{}", "no master entries configured — nothing to poll"),
                parsed: None,
            });
        }
    }

    pub fn set_error<T: Into<String>>(&mut self, msg: T) {
        self.error = Some((msg.into(), Local::now()));
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn init_subpage_form(&mut self) {
        if self.subpage_form.is_none() {
            self.subpage_form = Some(SubpageForm::default());
        }
        self.subpage_active = true;
    }

    fn perform_device_scan(&mut self) {
        // Placeholder: previously may have probed devices; now just stamp time.
        self.last_scan_info.clear();
        self.last_scan_time = Some(Local::now());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
}

const PORT_TOGGLE_MIN_INTERVAL_MS: u64 = 300; // throttle rapid toggles

// (Removed duplicate available_ports_sorted: now using crate::protocol::tty::available_ports_sorted())

// Re-export a helper for opening serial ports (previously sp_new wrapper)
fn sp_new(name: &str, baud: u32) -> serialport::SerialPortBuilder {
    serialport::new(name, baud)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterField {
    SlaveId,
    Mode,
    Address,
    Length,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditingField {
    Loop,
    Baud,
    Parity,
    StopBits,
    DataBits,
    RegisterField { idx: usize, field: RegisterField },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterEditField {
    Role,
    Id,
    Type,
    Start,
    End,
    Refresh,
    Counter,
    Value(u16),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
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
