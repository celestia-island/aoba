use chrono::Local;
use serialport::SerialPortInfo;
use std::time::Duration;

use crate::{
    i18n::lang,
    protocol::runtime::{PortRuntimeHandle, RuntimeCommand, SerialConfig},
    protocol::status::*,
};

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
            per_port_states: std::collections::HashMap::new(),
            per_port_slave_contexts: std::collections::HashMap::new(),
            last_scan_info: Vec::new(),
            last_scan_time: None,
            busy: false,
            spinner_frame: 0,
            polling_paused: false,
            last_port_toggle: None,
            port_toggle_min_interval_ms: PORT_TOGGLE_MIN_INTERVAL_MS,
            recent_auto_sent: std::collections::VecDeque::new(),
            recent_auto_requests: std::collections::VecDeque::new(),
            pending_sync_port: None,
        }
    }
    pub fn with_ports(ports: Vec<SerialPortInfo>) -> Self {
        let mut s = Self::new();
        s.ports = ports.clone();
        s.port_states = Self::detect_port_states(&s.ports);
        s.port_handles = s.ports.iter().map(|_| None).collect();
        s.port_runtimes = s.ports.iter().map(|_| None).collect();
        s
    }
    pub fn append_log(&mut self, entry: LogEntry) {
        const MAX: usize = 1000;
        self.logs.push(entry);
        if self.logs.len() > MAX {
            let excess = self.logs.len() - MAX;
            self.logs.drain(0..excess);
            if self.log_selected >= self.logs.len() {
                self.log_selected = self.logs.len().saturating_sub(1);
            }
        }
        if self.log_auto_scroll {
            if self.logs.is_empty() {
                self.log_view_offset = 0;
            } else {
                self.log_view_offset = self.logs.len().saturating_sub(1);
                self.log_selected = self.logs.len().saturating_sub(1);
            }
        }
    }
    pub fn set_error<T: Into<String>>(&mut self, msg: T) {
        self.error = Some((msg.into(), Local::now()));
    }
    pub fn clear_error(&mut self) {
        self.error = None;
    }
    pub fn current_serial_config(&self) -> Option<SerialConfig> {
        let form = self.subpage_form.as_ref()?;
        Some(SerialConfig {
            baud: form.baud,
            data_bits: form.data_bits,
            stop_bits: form.stop_bits,
            parity: form.parity,
        })
    }
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
    pub fn tick_spinner(&mut self) {
        if self.busy {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
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
    pub fn load_current_port_state(&mut self) {
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
    pub(crate) fn is_port_free(port_name: &str) -> bool {
        sp_new(port_name, 9600)
            .timeout(Duration::from_millis(50))
            .open()
            .is_ok()
    }
    fn detect_port_states(ports: &[SerialPortInfo]) -> Vec<PortState> {
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
    pub fn toggle_selected_port(&mut self) {
        let now = std::time::Instant::now();
        if let Some(last) = self.last_port_toggle {
            if now.duration_since(last).as_millis() < self.port_toggle_min_interval_ms as u128 {
                self.set_error(lang().protocol.common.toggle_too_fast.clone());
                return;
            }
        }
        self.busy = true;
        if self.ports.is_empty() {
            self.busy = false;
            return;
        }
        let i = self.selected;
        let special_base = self.ports.len();
        if i >= special_base {
            let rel = i - special_base;
            if rel == 0 {
                self.refresh();
            } else {
                self.set_error(
                    "Manual device specify: only supported on Linux and not implemented yet",
                );
            }
            return;
        }
        if let Some(state) = self.port_states.get_mut(i) {
            match state {
                PortState::Free => {
                    let port_name = self.ports[i].port_name.clone();
                    match sp_new(&port_name, 9600)
                        .timeout(Duration::from_millis(200))
                        .open()
                    {
                        Ok(handle) => {
                            if let Some(hslot) = self.port_handles.get_mut(i) {
                                *hslot = None;
                            }
                            *state = PortState::OccupiedByThis;
                            let cfg = self.current_serial_config().unwrap_or_default();
                            if let Ok(rt) = PortRuntimeHandle::from_existing(handle, cfg.clone()) {
                                if let Some(rslot) = self.port_runtimes.get_mut(i) {
                                    *rslot = Some(rt);
                                }
                            } else {
                                self.set_error(format!("failed to spawn runtime for {port_name}"));
                            }
                            self.last_port_toggle = Some(now);
                        }
                        Err(e) => {
                            *state = PortState::OccupiedByOther;
                            self.set_error(format!("failed to open {port_name}: {e}"));
                        }
                    }
                }
                PortState::OccupiedByThis => {
                    if let Some(hslot) = self.port_handles.get_mut(i) {
                        *hslot = None;
                    }
                    if let Some(rslot) = self.port_runtimes.get_mut(i) {
                        if let Some(rt) = rslot.take() {
                            let _ = rt.cmd_tx.send(RuntimeCommand::Stop);
                        }
                    }
                    *state = PortState::Free;
                    self.last_port_toggle = Some(now);
                }
                PortState::OccupiedByOther => {}
            }
        }
    }
    // Copy current subpage_form registers (Slave entries) into per-port ModbusStorageSmall
    pub fn sync_form_to_slave_context(&mut self, port_name: &str) {
        // Determine which form to read from: the currently selected (self.subpage_form)
        // or the saved per-port state's form.
        let form_opt: Option<&SubpageForm> = if let Some(info) = self.ports.get(self.selected) {
            if info.port_name == port_name {
                self.subpage_form.as_ref()
            } else {
                self.per_port_states
                    .get(port_name)
                    .and_then(|ps| ps.subpage_form.as_ref())
            }
        } else {
            self.per_port_states
                .get(port_name)
                .and_then(|ps| ps.subpage_form.as_ref())
        };
        let form = match form_opt {
            Some(f) => f,
            None => return,
        };
        let ctx = self
            .per_port_slave_contexts
            .entry(port_name.to_string())
            .or_insert_with(rmodbus::server::storage::ModbusStorageSmall::default);
        // Clear or keep existing? We'll overwrite affected ranges.
        // Copy all registers from the form into the per-port storage so the
        // simulated slave will reply based on the UI values. Previously this
        // only copied entries marked as Slave which left Master entries out and
        // caused zeroed responses.
        for reg in form.registers.iter() {
            let addr = reg.address as usize;
            let len = reg.length as usize;
            match reg.mode {
                RegisterMode::Holding => {
                    for i in 0..len {
                        if addr + i < ctx.holdings.len() && i < reg.values.len() {
                            ctx.holdings[addr + i] = reg.values[i];
                            log::debug!(
                                "sync: port={} holding[{}]={}",
                                port_name,
                                addr + i,
                                reg.values[i]
                            );
                        }
                    }
                }
                RegisterMode::Input => {
                    for i in 0..len {
                        if addr + i < ctx.inputs.len() && i < reg.values.len() {
                            ctx.inputs[addr + i] = reg.values[i];
                            log::debug!(
                                "sync: port={} input[{}]={}",
                                port_name,
                                addr + i,
                                reg.values[i]
                            );
                        }
                    }
                }
                RegisterMode::Coils => {
                    for i in 0..len {
                        if addr + i < ctx.coils.len() {
                            let v = if i < reg.values.len() {
                                reg.values[i] != 0
                            } else {
                                false
                            };
                            ctx.coils[addr + i] = v;
                            log::debug!("sync: port={} coil[{}]={}", port_name, addr + i, v);
                        }
                    }
                }
                RegisterMode::DiscreteInputs => {
                    for i in 0..len {
                        if addr + i < ctx.discretes.len() {
                            let v = if i < reg.values.len() {
                                reg.values[i] != 0
                            } else {
                                false
                            };
                            ctx.discretes[addr + i] = v;
                            log::debug!("sync: port={} discrete[{}]={}", port_name, addr + i, v);
                        }
                    }
                }
            }
        }
    }
    // init_subpage_form is implemented in `edit.rs` to avoid duplicate definitions
}
