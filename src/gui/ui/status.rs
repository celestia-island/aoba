use eframe::egui;
use egui::Ui;

use crate::i18n::lang;

/// Render bottom status content into an existing bottom panel UI.
pub fn render_status_ui(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(format!("{}: {}", lang().index.last, lang().index.last_none));
        ui.separator();
        ui.label(lang().index.auto_off.clone());
        ui.separator();
        ui.label("No errors");
    });
}
