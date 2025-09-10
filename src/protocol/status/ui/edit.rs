use chrono::Local;

use crate::protocol::status::*;

impl Status {
    pub fn pause_and_reset_slave_listen(&mut self) {
        if let Some(form) = self.ui.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                reg.req_success = 0;
                reg.req_total = 0;
                reg.pending_requests.clear();
                for v in reg.values.iter_mut() {
                    *v = 0;
                }
                reg.next_poll_at = std::time::Instant::now() + std::time::Duration::from_secs(3600);
            }
        }
        self.busy.polling_paused = true;
    }

    pub fn resume_slave_listen(&mut self) {
        let mut found_master = false;
        self.ui.logs.clear();
        self.ui.log_selected = 0;
        self.ui.log_view_offset = 0;
        if let Some(form) = self.ui.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                reg.next_poll_at = std::time::Instant::now();
                if reg.role == EntryRole::Master {
                    found_master = true;
                }
            }
        }
        // Sync current form values to per-port slave contexts so auto-slave uses TUI values
        if self.ui.subpage_form.is_some() {
            if let Some(info) = self.ports.list.get(self.ui.selected) {
                let pname = info.port_name.clone();
                self.sync_form_to_slave_context(pname.as_str());
            }
        }
        self.busy.polling_paused = false;
        if !found_master {
            // inline Status::append_log
            {
                const MAX: usize = 1000;
                self.ui.logs.push(LogEntry {
                    when: Local::now(),
                    raw: "no master entries configured — nothing to poll".into(),
                    parsed: None,
                });
                if self.ui.logs.len() > MAX {
                    let excess = self.ui.logs.len() - MAX;
                    self.ui.logs.drain(0..excess);
                    if self.ui.log_selected >= self.ui.logs.len() {
                        self.ui.log_selected = self.ui.logs.len().saturating_sub(1);
                    }
                }
                if self.ui.log_auto_scroll {
                    if self.ui.logs.is_empty() {
                        self.ui.log_view_offset = 0;
                    } else {
                        self.ui.log_view_offset = self.ui.logs.len().saturating_sub(1);
                        self.ui.log_selected = self.ui.logs.len().saturating_sub(1);
                    }
                }
            }
        }
    }

    pub fn init_subpage_form(&mut self) {
        if self.ui.subpage_form.is_none() {
            self.ui.subpage_form = Some(SubpageForm::default());
        }
        self.ui.subpage_active = true;
        // Ensure the page stack's top is a Modbus page so that per-port
        // snapshots capture the correct page variant. Older code sometimes
        // only updated flat ui fields which left the page stack as Entry;
        // when that Entry was saved and later restored the UI jumped back
        // to the homepage. Replace (or push) the top page with a Modbus
        // page reflecting the current flat fields.
        let modbus_page = crate::protocol::status::Page::Modbus {
            selected: self.ui.selected,
            subpage_active: self.ui.subpage_active,
            subpage_form: self.ui.subpage_form.clone(),
            subpage_tab_index: self.ui.subpage_tab_index,
            logs: self.ui.logs.clone(),
            log_selected: self.ui.log_selected,
            log_view_offset: self.ui.log_view_offset,
            log_auto_scroll: self.ui.log_auto_scroll,
            log_clear_pending: self.ui.log_clear_pending,
            input_mode: self.ui.input_mode,
            input_editing: self.ui.input_editing,
            input_buffer: self.ui.input_buffer.clone(),
            app_mode: self.ui.app_mode,
        };
        if self.ui.pages.is_empty() {
            self.ui.pages.push(modbus_page);
        } else {
            *self.ui.pages.last_mut().unwrap() = modbus_page;
        }
    }
}
