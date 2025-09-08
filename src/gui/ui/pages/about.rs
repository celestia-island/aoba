use eframe::egui;
use egui::{CentralPanel};
use crate::i18n::lang;
use crate::protocol::status::Status;
use crate::tui::utils::bus::Bus;
use eframe::Frame;

pub fn render_about(ctx: &egui::Context, _inner: &std::sync::Arc<std::sync::Mutex<Status>>, _bus: &Bus, _frame: &mut Frame) {
    CentralPanel::default().show(ctx, |ui| {
        ui.heading(lang().about.name.as_str());
        ui.separator();
        ui.label(lang().about.welcome.as_str());
    });
}
