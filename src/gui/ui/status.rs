use eframe::egui;
use egui::{Color32, TopBottomPanel};

use crate::{i18n::lang, protocol::status::Status};

pub fn render_status(
    ctx: &egui::Context,
    last_refresh: &Option<chrono::DateTime<chrono::Local>>,
    auto_refresh: bool,
    error: &Option<(String, chrono::DateTime<chrono::Local>)>,
    inner: &std::sync::Arc<std::sync::Mutex<Status>>,
) {
    TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Last refresh
            let last = if let Some(dt) = last_refresh {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                lang().last_none.clone()
            };
            ui.label(format!("{}: {}", lang().last, last));

            ui.separator();

            // Auto refresh status
            let auto_label = if auto_refresh {
                lang().auto_on.clone()
            } else {
                lang().auto_off.clone()
            };
            ui.label(auto_label);

            ui.separator();

            // Error area
            if let Some((msg, _ts)) = error {
                ui.colored_label(Color32::LIGHT_RED, msg);
                if ui.button(lang().press_c_clear.as_str()).clicked() {
                    if let Ok(mut guard) = inner.lock() {
                        guard.clear_error();
                    }
                }
            }
        });
    });
}
