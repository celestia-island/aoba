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
    if let Ok((is_empty, _idx, p)) = crate::protocol::status::status_rw::read_status(inner, |g| {
        if g.ports.list.is_empty() {
            Ok((true, 0usize, None))
        } else {
            let idx = if g.ui.selected >= g.ports.list.len() {
                0usize
            } else {
                g.ui.selected
            };
            Ok((false, idx, Some(g.ports.list[idx].clone())))
        }
    }) {
        if is_empty {
            ui.label(lang().index.no_com_ports.as_str());
        } else if let Some(p) = p {
            ui.label(format!("{} {}", lang().index.name_label, p.port_name));
            ui.label(format!("{} {:?}", lang().index.type_label, p.port_type));
            ui.separator();
            ui.label(lang().index.details_placeholder.as_str());
        }
    }
}
