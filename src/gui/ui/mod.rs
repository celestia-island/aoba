pub mod components;
pub mod pages;
pub mod status;


use eframe::{egui::Context, Frame};
use egui::TopBottomPanel;

use crate::tui::utils::bus::Bus;

/// Centralized UI renderer: title, breadcrumb navigation, pages and bottom status.
pub fn render_ui(ctx: &Context, frame: &mut Frame, bus: &Bus) {
    // Top panel (title + breadcrumbs)
    TopBottomPanel::top("title_panel")
        .default_height(56.)
        .resizable(false)
        .show(ctx, |ui| {
            let _ = components::title::render_title_ui(ui);
        });

    // Central area: delegate to per-page renderers
    pages::render_panels(ctx, bus, frame);

    // Bottom status panel
    TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
        status::render_status_ui(ui);
    });
}
