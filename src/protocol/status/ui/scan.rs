use chrono::Local;

use crate::{
    protocol::runtime::{PortRuntimeHandle, RuntimeCommand},
    protocol::status::{ui as ui_accessors, *},
};

impl Status {
    pub fn refresh(&mut self) {
        self.busy.busy = true;
        // remember the selected index at function entry; we'll only restore
        // the per-port snapshot if the selection actually changed by the
        // refresh operation.
        let old_selected = self.ui.selected;
        // inline save_current_port_state
        if self.ui.selected < self.ports.list.len() {
            if let Some(info) = self.ports.list.get(self.ui.selected) {
                let snap = PerPortState {
                    subpage_active: self.ui.subpage_active,
                    subpage_form: self.ui.subpage_form.clone(),
                    subpage_tab_index: self.ui.subpage_tab_index,
                    logs: crate::protocol::status::ui::ui_logs_get(&self),
                    log_selected: self.ui.log_selected,
                    log_view_offset: self.ui.log_view_offset,
                    log_auto_scroll: self.ui.log_auto_scroll,
                    log_clear_pending: self.ui.log_clear_pending,
                    input_mode: self.ui.input_mode,
                    input_editing: self.ui.input_editing,
                    input_buffer: self.ui.input_buffer.clone(),
                    app_mode: self.ui.app_mode,
                    page: ui_accessors::ui_pages_last_get(&self),
                };
                self.per_port.states.insert(info.port_name.clone(), snap);
            }
        }
        // inline perform_device_scan
        self.scan.last_scan_info.clear();
        self.scan.last_scan_time = Some(Local::now());
        let enriched = crate::protocol::tty::available_ports_enriched();
        let new_ports: Vec<_> = enriched.iter().map(|(p, _)| p.clone()).collect();
        let new_extras: Vec<_> = enriched.into_iter().map(|(_, e)| e).collect();
        let prev_selected_name =
            if !self.ports.list.is_empty() && self.ui.selected < self.ports.list.len() {
                Some(self.ports.list[self.ui.selected].port_name.clone())
            } else {
                None
            };
        // If selection pointed to a virtual entry, remember its relative index so we can restore it after refresh
        let prev_selected_virtual_rel =
            if !self.ports.list.is_empty() && self.ui.selected >= self.ports.list.len() {
                Some(self.ui.selected - self.ports.list.len())
            } else {
                None
            };
        let mut name_to_state: std::collections::HashMap<String, PortState> =
            std::collections::HashMap::new();
        let mut name_to_handle: std::collections::HashMap<String, Option<SerialPortWrapper>> =
            std::collections::HashMap::new();
        let mut name_to_runtime: std::collections::HashMap<String, Option<PortRuntimeHandle>> =
            std::collections::HashMap::new();
        for (i, p) in self.ports.list.iter().enumerate() {
            if let Some(s) = self.ports.states.get(i) {
                name_to_state.insert(p.port_name.clone(), *s);
            }
            if let Some(h) = self.ports.handles.get_mut(i) {
                let taken = h.take();
                name_to_handle.insert(p.port_name.clone(), taken);
            }
            if let Some(r) = self.ports.runtimes.get_mut(i) {
                let taken = r.take();
                name_to_runtime.insert(p.port_name.clone(), taken);
            }
        }
        self.ports.list = new_ports;
        self.ports.extras = new_extras;
        let mut new_states = Vec::with_capacity(self.ports.list.len());
        let mut new_handles = Vec::with_capacity(self.ports.list.len());
        for p in self.ports.list.iter() {
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
        self.ports.states = new_states;
        self.ports.handles = new_handles;
        self.ports.runtimes = self
            .ports
            .list
            .iter()
            .map(|p| name_to_runtime.remove(&p.port_name).unwrap_or(None))
            .collect();
        for (_name, rt_opt) in name_to_runtime.into_iter() {
            if let Some(rt) = rt_opt {
                let _ = rt.cmd_tx.send(RuntimeCommand::Stop);
            }
        }
        if self.ports.list.is_empty() {
            self.ui.selected = 0;
        } else {
            if let Some(name) = prev_selected_name {
                if let Some(idx) = self.ports.list.iter().position(|p| p.port_name == name) {
                    self.ui.selected = idx;
                }
            } else if let Some(rel) = prev_selected_virtual_rel {
                // restore virtual selection (ensure within bounds of new extras)
                let extras_len = self.ports.extras.len();
                if rel < extras_len {
                    self.ui.selected = self.ports.list.len() + rel;
                } else if extras_len > 0 {
                    self.ui.selected = self.ports.list.len() + extras_len - 1;
                } else {
                    self.ui.selected = 0;
                }
            }
            // ports + Refresh + Manual + About = ports + 3 virtual entries
            let total = self.ports.list.len().saturating_add(3);
            if self.ui.selected >= total {
                self.ui.selected = 0;
            }
        }
        self.ui.last_refresh = Some(Local::now());
        // Only restore per-port snapshot if the selected index changed.
        if self.ui.selected != old_selected {
            // inline load_current_port_state
            if self.ui.selected < self.ports.list.len() {
                if let Some(info) = self.ports.list.get(self.ui.selected) {
                    if let Some(snap) = self.per_port.states.get(&info.port_name).cloned() {
                        if let Some(page) = snap.page {
                            if self.ui.pages.is_empty() {
                                self.ui.pages.push(page);
                            } else {
                                *self.ui.pages.last_mut().unwrap() = page;
                            }
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
                            crate::protocol::status::ui::ui_log_selected_set(
                                self,
                                snap.log_selected,
                            );
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
                        self.ui.subpage_tab_index = crate::protocol::status::SubpageTab::Config;
                        crate::protocol::status::ui::ui_logs_set(self, Vec::new());
                        crate::protocol::status::ui::ui_log_selected_set(self, 0);
                        crate::protocol::status::ui::ui_log_view_offset_set(self, 0);
                        crate::protocol::status::ui::ui_log_auto_scroll_set(self, true);
                        self.ui.input_mode = crate::protocol::status::InputMode::Ascii;
                        self.ui.input_editing = false;
                        self.ui.input_buffer.clear();
                        self.ui.app_mode = crate::protocol::status::AppMode::Modbus;
                    }
                }
            }
        }
        self.busy.busy = false;
    }

    pub fn refresh_ports_only(&mut self) {
        self.busy.busy = true;
        let old_selected = self.ui.selected;
        self.save_current_port_state();
        let enriched = crate::protocol::tty::available_ports_enriched();
        let new_ports: Vec<_> = enriched.iter().map(|(p, _)| p.clone()).collect();
        let new_extras: Vec<_> = enriched.into_iter().map(|(_, e)| e).collect();
        let prev_selected_name =
            if !self.ports.list.is_empty() && self.ui.selected < self.ports.list.len() {
                Some(self.ports.list[self.ui.selected].port_name.clone())
            } else {
                None
            };
        let mut name_to_state: std::collections::HashMap<String, PortState> =
            std::collections::HashMap::new();
        let mut name_to_handle: std::collections::HashMap<String, Option<SerialPortWrapper>> =
            std::collections::HashMap::new();
        let mut name_to_runtime: std::collections::HashMap<String, Option<PortRuntimeHandle>> =
            std::collections::HashMap::new();
        for (i, p) in self.ports.list.iter().enumerate() {
            if let Some(s) = self.ports.states.get(i) {
                name_to_state.insert(p.port_name.clone(), *s);
            }
            if let Some(h) = self.ports.handles.get_mut(i) {
                let taken = h.take();
                name_to_handle.insert(p.port_name.clone(), taken);
            }
            if let Some(r) = self.ports.runtimes.get_mut(i) {
                let taken = r.take();
                name_to_runtime.insert(p.port_name.clone(), taken);
            }
        }
        self.ports.list = new_ports;
        self.ports.extras = new_extras;
        let mut new_states = Vec::with_capacity(self.ports.list.len());
        let mut new_handles = Vec::with_capacity(self.ports.list.len());
        for p in self.ports.list.iter() {
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
        self.ports.states = new_states;
        self.ports.handles = new_handles;
        self.ports.runtimes = self
            .ports
            .list
            .iter()
            .map(|p| name_to_runtime.remove(&p.port_name).unwrap_or(None))
            .collect();
        for (_name, rt_opt) in name_to_runtime.into_iter() {
            if let Some(rt) = rt_opt {
                let _ = rt.cmd_tx.send(RuntimeCommand::Stop);
            }
        }
        if self.ports.list.is_empty() {
            self.ui.selected = 0;
        } else {
            if let Some(name) = prev_selected_name {
                if let Some(idx) = self.ports.list.iter().position(|p| p.port_name == name) {
                    self.ui.selected = idx;
                }
            }
            // ports + Refresh + Manual + About = ports + 3 virtual entries
            let total = self.ports.list.len().saturating_add(3);
            if self.ui.selected >= total {
                self.ui.selected = 0;
            }
        }
        self.ui.last_refresh = Some(Local::now());
        if self.ui.selected != old_selected {
            self.load_current_port_state();
        }
        self.busy.busy = false;
    }

    // quick_scan / adjust_log_view / page_up / page_down / perform_device_scan 已被内联到调用点
}
