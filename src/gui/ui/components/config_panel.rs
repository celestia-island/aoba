use eframe::egui::Ui;

// Minimal egui stub for config panel used by GUI pages.
// This is intentionally lightweight: it provides a placeholder render function with
// a compatible name so existing callers can be adapted later.

pub fn render_config_panel(ui: &mut Ui) {
    ui.label("[config panel placeholder]");
}
