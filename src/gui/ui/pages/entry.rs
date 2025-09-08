use crate::tui::utils::bus::Bus;
use crate::{i18n::lang, protocol::status::Status};
use eframe::Frame;
use egui::Ui;

pub fn render_entry_ui(
    ui: &mut Ui,
    inner: &std::sync::Arc<std::sync::Mutex<Status>>,
    _bus: &Bus,
    _frame: &mut Frame,
) {
    ui.heading(lang().index.details.as_str());
    ui.separator();
    if let Ok(guard) = inner.lock() {
        if guard.ports.list.is_empty() {
            ui.label(lang().index.no_com_ports.as_str());
        } else {
            let idx = if guard.ui.selected >= guard.ports.list.len() {
                0
            } else {
                guard.ui.selected
            };
            let p = &guard.ports.list[idx];
            ui.label(format!("{} {}", lang().index.name_label, p.port_name));
            ui.label(format!("{} {:?}", lang().index.type_label, p.port_type));
            ui.separator();
            ui.label(lang().index.details_placeholder.as_str());
        }
    }
}
