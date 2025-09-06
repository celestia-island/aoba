use chrono::Local;
use serialport::SerialPort;
use std::collections::HashMap;

use crate::{
    protocol::runtime::{PortRuntimeHandle, RuntimeCommand},
    protocol::status::*,
};

impl Status {
    pub fn refresh(&mut self) {
        self.busy = true;
        self.save_current_port_state();
        self.perform_device_scan();
        let enriched = crate::protocol::tty::available_ports_enriched();
        let new_ports: Vec<_> = enriched.iter().map(|(p, _)| p.clone()).collect();
        let new_extras: Vec<_> = enriched.into_iter().map(|(_, e)| e).collect();
        let prev_selected_name = if !self.ports.is_empty() && self.selected < self.ports.len() {
            Some(self.ports[self.selected].port_name.clone())
        } else {
            None
        };
        // If selection pointed to a virtual entry, remember its relative index so we can restore it after refresh
        let prev_selected_virtual_rel =
            if !self.ports.is_empty() && self.selected >= self.ports.len() {
                Some(self.selected - self.ports.len())
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
        let mut new_states = Vec::with_capacity(self.ports.len());
        let mut new_handles = Vec::with_capacity(self.ports.len());
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
            } else if let Some(rel) = prev_selected_virtual_rel {
                // restore virtual selection (ensure within bounds of new extras)
                let extras_len = self.port_extras.len();
                if rel < extras_len {
                    self.selected = self.ports.len() + rel;
                } else if extras_len > 0 {
                    self.selected = self.ports.len() + extras_len - 1;
                } else {
                    self.selected = 0;
                }
            }
            // ports + Refresh + Manual + About = ports + 3 virtual entries
            let total = self.ports.len().saturating_add(3);
            if self.selected >= total {
                self.selected = 0;
            }
        }
        self.last_refresh = Some(Local::now());
        self.load_current_port_state();
        self.busy = false;
    }

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
        let mut new_states = Vec::with_capacity(self.ports.len());
        let mut new_handles = Vec::with_capacity(self.ports.len());
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
            // ports + Refresh + Manual + About = ports + 3 virtual entries
            let total = self.ports.len().saturating_add(3);
            if self.selected >= total {
                self.selected = 0;
            }
        }
        self.last_refresh = Some(Local::now());
        self.load_current_port_state();
        self.busy = false;
    }

    pub fn quick_scan(&mut self) {
        self.perform_device_scan();
    }

    pub fn adjust_log_view(&mut self, term_height: u16) {
        if self.logs.is_empty() {
            return;
        }
        let bottom_len = if self.error.is_some() || self.subpage_active {
            2
        } else {
            1
        };
        let logs_area_h = (term_height as usize).saturating_sub(bottom_len + 5);
        let inner_h = logs_area_h.saturating_sub(2);
        let groups_per_screen =
            std::cmp::max(1usize, inner_h / crate::protocol::status::LOG_GROUP_HEIGHT);
        let bottom = if self.log_auto_scroll {
            self.logs.len().saturating_sub(1)
        } else {
            std::cmp::min(self.log_view_offset, self.logs.len().saturating_sub(1))
        };
        let top = (bottom + 1).saturating_sub(groups_per_screen);
        if self.log_selected < top {
            self.log_auto_scroll = false;
            let half = groups_per_screen / 2;
            let new_bottom =
                std::cmp::min(self.logs.len().saturating_sub(1), self.log_selected + half);
            self.log_view_offset = new_bottom;
        } else if self.log_selected > bottom {
            self.log_auto_scroll = false;
            self.log_view_offset = self.log_selected;
        }
    }

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
    pub fn page_down(&mut self, page: usize) {
        if self.logs.is_empty() {
            return;
        }
        let max_bottom = self.logs.len().saturating_sub(1);
        let new_bottom = (self.log_view_offset).saturating_add(page);
        self.log_view_offset = std::cmp::min(max_bottom, new_bottom);
        self.log_auto_scroll = self.log_view_offset >= max_bottom;
    }

    fn perform_device_scan(&mut self) {
        self.last_scan_info.clear();
        self.last_scan_time = Some(Local::now());
    }
}
