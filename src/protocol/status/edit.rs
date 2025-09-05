use chrono::Local;

use crate::protocol::status::*;

impl Status {
    pub fn pause_and_reset_slave_listen(&mut self) {
        if let Some(form) = self.subpage_form.as_mut() {
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
        self.polling_paused = true;
    }
    pub fn resume_slave_listen(&mut self) {
        let mut found_master = false;
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
        // Sync current form values to per-port slave contexts so auto-slave uses TUI values
        if self.subpage_form.is_some() {
            if let Some(info) = self.ports.get(self.selected) {
                let pname = info.port_name.clone();
                self.sync_form_to_slave_context(pname.as_str());
            }
        }
        self.polling_paused = false;
        if !found_master {
            self.append_log(LogEntry {
                when: Local::now(),
                raw: "no master entries configured — nothing to poll".into(),
                parsed: None,
            });
        }
    }
    pub fn init_subpage_form(&mut self) {
        if self.subpage_form.is_none() {
            self.subpage_form = Some(SubpageForm::default());
        }
        self.subpage_active = true;
    }
}
