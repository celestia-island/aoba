use eframe::egui;
use egui::TopBottomPanel;

use crate::i18n::lang;

pub fn render_title(ctx: &egui::Context) {
    TopBottomPanel::top("title_panel").show(ctx, |ui| {
        ui.horizontal_centered(|ui| {
            ui.heading(lang().index.title.as_str());
        });
    });
}
