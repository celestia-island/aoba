use chrono::Local;
use std::time::Duration;

use serialport::SerialPortInfo;

use crate::{
    i18n::lang,
    protocol::runtime::{PortRuntimeHandle, RuntimeCommand, SerialConfig},
    protocol::status::*,
};

impl Status {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ports(ports: Vec<SerialPortInfo>) -> Self {
        let mut s = Self::new();
        s.ports.list = ports.clone();
        s.ports.states = Self::detect_port_states(&s.ports.list);
        s.ports.handles = s.ports.list.iter().map(|_| None).collect();
        s.ports.runtimes = s.ports.list.iter().map(|_| None).collect();
        s
    }

    pub fn append_log(&mut self, entry: LogEntry) {
        const MAX: usize = 1000;
        // Use compatibility accessors: get-modify-set to avoid long borrows and
        // centralize UI mutations through the accessor layer.
        let mut logs = crate::protocol::status::ui::ui_logs_get(&self);
        logs.push(entry);
        if logs.len() > MAX {
            let excess = logs.len() - MAX;
            logs.drain(0..excess);
        }
        let len = logs.len();
        crate::protocol::status::ui::ui_logs_set(self, logs);

        if crate::protocol::status::ui::ui_log_auto_scroll_get(&self) {
            if len == 0 {
                crate::protocol::status::ui::ui_log_view_offset_set(self, 0);
            } else {
                let last = len.saturating_sub(1);
                crate::protocol::status::ui::ui_log_view_offset_set(self, last);
                crate::protocol::status::ui::ui_log_selected_set(self, last);
            }
        }
    }

    pub fn set_error<T: Into<String>>(&mut self, msg: T) {
        crate::protocol::status::ui::ui_error_set(self, Some((msg.into(), Local::now())));
    }

    pub fn clear_error(&mut self) {
        crate::protocol::status::ui::ui_error_set(self, None);
    }

    pub fn current_serial_config(&self) -> Option<SerialConfig> {
        let form = self.ui.subpage_form.as_ref()?;
        Some(SerialConfig {
            baud: form.baud,
            data_bits: form.data_bits,
            stop_bits: form.stop_bits,
            parity: form.parity,
        })
    }

    pub fn sync_runtime_configs(&mut self) {
        if self.ui.selected >= self.ports.list.len() {
            return;
        }
        if let Some(Some(rt)) = self.ports.runtimes.get(self.ui.selected) {
            if let Some(new_cfg) = self.current_serial_config() {
                if new_cfg != rt.current_cfg {
                    let _ = rt.cmd_tx.send(RuntimeCommand::Reconfigure(new_cfg.clone()));
                    if let Some(rtm) = self
                        .ports
                        .runtimes
                        .get_mut(self.ui.selected)
                        .and_then(|o| o.as_mut())
                    {
                        rtm.current_cfg = new_cfg;
                    }
                }
            }
        }
        self.busy.busy = false;
    }

    pub fn tick_spinner(&mut self) {
        if self.busy.busy {
            self.busy.spinner_frame = self.busy.spinner_frame.wrapping_add(1);
        }
    }

    pub fn toggle_auto_refresh(&mut self) {
        self.ui.auto_refresh = !self.ui.auto_refresh;
    }

    pub fn next(&mut self) {
        let total = self.ports.list.len();
        if total == 0 {
            return;
        }
        self.save_current_port_state();
        self.ui.selected = (self.ui.selected + 1) % total;
        self.load_current_port_state();
    }

    pub fn prev(&mut self) {
        let total = self.ports.list.len();
        if total == 0 {
            return;
        }
        self.save_current_port_state();
        if self.ui.selected == 0 {
            self.ui.selected = total - 1;
        } else {
            self.ui.selected -= 1;
        }
        self.load_current_port_state();
    }

    pub fn next_visual(&mut self) {
        // ports + Refresh + Manual + About = ports + 3 virtual entries
        let total = self.ports.list.len().saturating_add(3);
        if total == 0 {
            return;
        }
        let was_real = self.ui.selected < self.ports.list.len();
        if was_real {
            self.save_current_port_state();
        }
        self.ui.selected = (self.ui.selected + 1) % total;
        if self.ui.selected < self.ports.list.len() {
            self.load_current_port_state();
        }
    }

    pub fn prev_visual(&mut self) {
        // ports + Refresh + Manual + About = ports + 3 virtual entries
        let total = self.ports.list.len().saturating_add(3);
        if total == 0 {
            return;
        }
        let was_real = self.ui.selected < self.ports.list.len();
        if was_real {
            self.save_current_port_state();
        }
        if self.ui.selected == 0 {
            self.ui.selected = total - 1;
        } else {
            self.ui.selected -= 1;
        }
        if self.ui.selected < self.ports.list.len() {
            self.load_current_port_state();
        }
    }

    pub fn save_current_port_state(&mut self) {
        if self.ui.selected < self.ports.list.len() {
            if let Some(info) = self.ports.list.get(self.ui.selected) {
                let snap = PerPortState {
                    subpage_active: self.ui.subpage_active,
                    subpage_form: self.ui.subpage_form.clone(),
                    subpage_tab_index: self.ui.subpage_tab_index,
                    logs: crate::protocol::status::ui::ui_logs_get(&self),
                    log_selected: crate::protocol::status::ui::ui_log_selected_get(&self),
                    log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(&self),
                    log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(&self),
                    log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(&self),
                    input_mode: self.ui.input_mode,
                    input_editing: self.ui.input_editing,
                    input_buffer: crate::protocol::status::ui::ui_input_buffer_get(&self),
                    app_mode: self.ui.app_mode,
                    page: crate::protocol::status::ui::ui_pages_last_get(&self),
                };
                self.per_port.states.insert(info.port_name.clone(), snap);
            }
        }
    }

    pub fn load_current_port_state(&mut self) {
        if self.ui.selected < self.ports.list.len() {
            if let Some(info) = self.ports.list.get(self.ui.selected) {
                if let Some(snap) = self.per_port.states.get(&info.port_name).cloned() {
                    // If we have a full page snapshot, restore it to the page
                    // stack so page-specific UI is preserved. Otherwise fall
                    // back to restoring the flat ui fields.
                    if let Some(page) = snap.page {
                        // replace current top page with the saved one
                        if self.ui.pages.is_empty() {
                            self.ui.pages.push(page);
                        } else {
                            *self.ui.pages.last_mut().unwrap() = page;
                        }
                        // ensure flat fields reflect the restored page (inlined sync_ui_from_page)
                        match self.ui.pages.last().cloned().unwrap_or_default() {
                            crate::protocol::status::Page::Entry {
                                selected,
                                input_mode,
                                input_editing,
                                input_buffer,
                                app_mode,
                            } => {
                                self.ui.selected = selected;
                                self.ui.input_mode = input_mode;
                                self.ui.input_editing = input_editing;
                                self.ui.input_buffer = input_buffer;
                                self.ui.app_mode = app_mode;
                                self.ui.subpage_active = false;
                                self.ui.subpage_form = None;
                            }
                            crate::protocol::status::Page::Modbus {
                                selected,
                                subpage_active,
                                subpage_form,
                                subpage_tab_index,
                                logs,
                                log_selected,
                                log_view_offset,
                                log_auto_scroll,
                                log_clear_pending,
                                input_mode,
                                input_editing,
                                input_buffer,
                                app_mode,
                            } => {
                                self.ui.selected = selected;
                                self.ui.subpage_active = subpage_active;
                                self.ui.subpage_form = subpage_form;
                                self.ui.subpage_tab_index = subpage_tab_index;
                                crate::protocol::status::ui::ui_logs_set(self, logs);
                                crate::protocol::status::ui::ui_log_selected_set(
                                    self,
                                    log_selected,
                                );
                                crate::protocol::status::ui::ui_log_view_offset_set(
                                    self,
                                    log_view_offset,
                                );
                                crate::protocol::status::ui::ui_log_auto_scroll_set(
                                    self,
                                    log_auto_scroll,
                                );
                                crate::protocol::status::ui::ui_log_clear_pending_set(
                                    self,
                                    log_clear_pending,
                                );
                                self.ui.input_mode = input_mode;
                                self.ui.input_editing = input_editing;
                                self.ui.input_buffer = input_buffer;
                                self.ui.app_mode = app_mode;
                            }
                        }
                    } else {
                        self.ui.subpage_active = snap.subpage_active;
                        self.ui.subpage_form = snap.subpage_form;
                        self.ui.subpage_tab_index = snap.subpage_tab_index;
                        crate::protocol::status::ui::ui_logs_set(self, snap.logs);
                        crate::protocol::status::ui::ui_log_selected_set(self, snap.log_selected);
                        crate::protocol::status::ui::ui_log_view_offset_set(
                            self,
                            snap.log_view_offset,
                        );
                        crate::protocol::status::ui::ui_log_auto_scroll_set(
                            self,
                            snap.log_auto_scroll,
                        );
                        crate::protocol::status::ui::ui_log_clear_pending_set(
                            self,
                            snap.log_clear_pending,
                        );
                        self.ui.input_mode = snap.input_mode;
                        self.ui.input_editing = snap.input_editing;
                        self.ui.input_buffer = snap.input_buffer;
                        self.ui.app_mode = snap.app_mode;
                        // also ensure page reflects these flat fields (inlined sync_page_from_ui)
                        if self.ui.pages.is_empty() {
                            self.ui.pages.push(crate::protocol::status::Page::default());
                        }
                        match self.ui.pages.last_mut().unwrap() {
                            crate::protocol::status::Page::Entry {
                                selected,
                                input_mode,
                                input_editing,
                                input_buffer,
                                app_mode,
                            } => {
                                *selected = self.ui.selected;
                                *input_mode = self.ui.input_mode;
                                *input_editing = self.ui.input_editing;
                                *input_buffer = self.ui.input_buffer.clone();
                                *app_mode = self.ui.app_mode;
                            }
                            crate::protocol::status::Page::Modbus {
                                selected,
                                subpage_active,
                                subpage_form,
                                subpage_tab_index,
                                logs,
                                log_selected,
                                log_view_offset,
                                log_auto_scroll,
                                log_clear_pending,
                                input_mode,
                                input_editing,
                                input_buffer,
                                app_mode,
                            } => {
                                *selected = self.ui.selected;
                                *subpage_active = self.ui.subpage_active;
                                *subpage_form = self.ui.subpage_form.clone();
                                *subpage_tab_index = self.ui.subpage_tab_index;
                                *logs = self.ui.logs.clone();
                                *log_selected = self.ui.log_selected;
                                *log_view_offset = self.ui.log_view_offset;
                                *log_auto_scroll = self.ui.log_auto_scroll;
                                *log_clear_pending = self.ui.log_clear_pending;
                                *input_mode = self.ui.input_mode;
                                *input_editing = self.ui.input_editing;
                                *input_buffer = self.ui.input_buffer.clone();
                                *app_mode = self.ui.app_mode;
                            }
                        }
                    }
                } else {
                    self.ui.subpage_active = false;
                    self.ui.subpage_form = None;
                    self.ui.subpage_tab_index = SubpageTab::Config;
                    crate::protocol::status::ui::ui_logs_set(self, Vec::new());
                    crate::protocol::status::ui::ui_log_selected_set(self, 0);
                    crate::protocol::status::ui::ui_log_view_offset_set(self, 0);
                    crate::protocol::status::ui::ui_log_auto_scroll_set(self, true);
                    self.ui.input_mode = InputMode::Ascii;
                    self.ui.input_editing = false;
                    crate::protocol::status::ui::ui_input_buffer_set(self, String::new());
                    self.ui.app_mode = AppMode::Modbus;
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
        if let Some(last) = self.toggles.last_port_toggle {
            if now.duration_since(last).as_millis()
                < self.toggles.port_toggle_min_interval_ms as u128
            {
                self.set_error(lang().protocol.common.toggle_too_fast.clone());
                return;
            }
        }
        self.busy.busy = true;
        if self.ports.list.is_empty() {
            self.busy.busy = false;
            return;
        }
        let i = self.ui.selected;
        let special_base = self.ports.list.len();
        if i >= special_base {
            let rel = i - special_base;
            match rel {
                0 => {
                    self.refresh();
                }
                1 => {
                    self.set_error(
                        "Manual device specify: only supported on Linux and not implemented yet",
                    );
                }
                2 => {
                    // About virtual entry: open About full-page subpage.
                    self.ui.subpage_active = true;
                }
                _ => {}
            }
            return;
        }
        if let Some(state) = self.ports.states.get_mut(i) {
            match state {
                PortState::Free => {
                    let port_name = self.ports.list[i].port_name.clone();
                    match sp_new(&port_name, 9600)
                        .timeout(Duration::from_millis(200))
                        .open()
                    {
                        Ok(handle) => {
                            if let Some(hslot) = self.ports.handles.get_mut(i) {
                                *hslot = None;
                            }
                            *state = PortState::OccupiedByThis;
                            let cfg = self.current_serial_config().unwrap_or_default();
                            if let Ok(rt) = PortRuntimeHandle::from_existing(handle, cfg.clone()) {
                                if let Some(rslot) = self.ports.runtimes.get_mut(i) {
                                    *rslot = Some(rt);
                                }
                            } else {
                                self.set_error(format!("failed to spawn runtime for {port_name}"));
                            }
                            self.toggles.last_port_toggle = Some(now);
                        }
                        Err(e) => {
                            *state = PortState::OccupiedByOther;
                            self.set_error(format!("failed to open {port_name}: {e}"));
                        }
                    }
                }
                PortState::OccupiedByThis => {
                    if let Some(hslot) = self.ports.handles.get_mut(i) {
                        *hslot = None;
                    }
                    if let Some(rslot) = self.ports.runtimes.get_mut(i) {
                        if let Some(rt) = rslot.take() {
                            let _ = rt.cmd_tx.send(RuntimeCommand::Stop);
                        }
                    }
                    *state = PortState::Free;
                    self.toggles.last_port_toggle = Some(now);
                }
                PortState::OccupiedByOther => {}
            }
        }
    }
    // Copy current subpage_form registers (Slave entries) into per-port ModbusStorageSmall
    pub fn sync_form_to_slave_context(&mut self, port_name: &str) {
        // Determine which form to read from: the currently selected (self.subpage_form)
        // or the saved per-port state's form.
        let form_opt: Option<&SubpageForm> =
            if let Some(info) = self.ports.list.get(self.ui.selected) {
                if info.port_name == port_name {
                    self.ui.subpage_form.as_ref()
                } else {
                    self.per_port
                        .states
                        .get(port_name)
                        .and_then(|ps| ps.subpage_form.as_ref())
                }
            } else {
                self.per_port
                    .states
                    .get(port_name)
                    .and_then(|ps| ps.subpage_form.as_ref())
            };
        let form = match form_opt {
            Some(f) => f,
            None => return,
        };
        let ctx_arc = self
            .per_port
            .slave_contexts
            .entry(port_name.to_string())
            .or_insert_with(|| {
                std::sync::Arc::new(std::sync::Mutex::new(
                    rmodbus::server::storage::ModbusStorageSmall::default(),
                ))
            });
        // lock the context for mutation
        let mut ctx = ctx_arc.lock().unwrap();
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
