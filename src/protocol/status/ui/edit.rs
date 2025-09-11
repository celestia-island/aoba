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
        // clear logs via accessor
        crate::protocol::status::ui::ui_logs_set(self, Vec::new());
        crate::protocol::status::ui::ui_log_selected_set(self, 0);
        crate::protocol::status::ui::ui_log_view_offset_set(self, 0);
        if let Some(form) = self.ui.subpage_form.as_mut() {
            for reg in form.registers.iter_mut() {
                reg.next_poll_at = std::time::Instant::now();
                if reg.role == EntryRole::Master {
                    found_master = true;
                }
            }
        }
        // Sync current form values to per-port slave contexts so auto-slave uses TUI values
        if crate::protocol::status::ui::ui_subpage_form_get(self).is_some() {
            if let Some(info) = self
                .ports
                .list
                .get(crate::protocol::status::ui::ui_selected_get(self))
            {
                let pname = info.port_name.clone();
                self.sync_form_to_slave_context(pname.as_str());
            }
        }
        self.busy.polling_paused = false;
        if !found_master {
            // inline Status::append_log via accessors
            {
                const MAX: usize = 1000;
                let mut logs = crate::protocol::status::ui::ui_logs_get(&self);
                logs.push(LogEntry {
                    when: Local::now(),
                    raw: "no master entries configured — nothing to poll".into(),
                    parsed: None,
                });
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
        }
    }

    pub fn init_subpage_form(&mut self) {
        if crate::protocol::status::ui::ui_subpage_form_get(self).is_none() {
            crate::protocol::status::ui::ui_subpage_form_set(self, Some(SubpageForm::default()));
        }
        crate::protocol::status::ui::ui_subpage_active_set(self, true);
        // Ensure the page stack's top is a Modbus page so that per-port
        // snapshots capture the correct page variant. Older code sometimes
        // only updated flat ui fields which left the page stack as Entry;
        // when that Entry was saved and later restored the UI jumped back
        // to the homepage. Replace (or push) the top page with a Modbus
        // page reflecting the current flat fields.
        let modbus_page = crate::protocol::status::Page::Modbus {
            selected: crate::protocol::status::ui::ui_selected_get(&self),
            subpage_active: crate::protocol::status::ui::ui_subpage_active_get(&self),
            subpage_form: crate::protocol::status::ui::ui_subpage_form_get(&self),
            subpage_tab_index: crate::protocol::status::ui::ui_subpage_tab_index_get(&self),
            logs: crate::protocol::status::ui::ui_logs_get(&self),
            log_selected: crate::protocol::status::ui::ui_log_selected_get(&self),
            log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(&self),
            log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(&self),
            log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(&self),
            input_mode: crate::protocol::status::ui::ui_input_mode_get(&self),
            input_editing: crate::protocol::status::ui::ui_input_editing_get(&self),
            input_buffer: crate::protocol::status::ui::ui_input_buffer_get(&self),
            app_mode: crate::protocol::status::ui::ui_app_mode_get(&self),
        };
        if self.ui.pages.is_empty() {
            self.ui.pages.push(modbus_page);
        } else {
            *self.ui.pages.last_mut().unwrap() = modbus_page;
        }
    }
}
