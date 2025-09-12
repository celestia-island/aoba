pub mod components;
pub mod pages;
pub mod status;

use std::sync::{Arc, RwLock};

use eframe::{egui::Context, Frame};
use egui::TopBottomPanel;

use crate::{protocol::status::Status, tui::utils::bus::Bus};

/// Centralized UI renderer: title, breadcrumb navigation, pages and bottom status.
pub fn render_ui(ctx: &Context, frame: &mut Frame, inner: &Arc<RwLock<Status>>, bus: &Bus) {
    // Top panel (title + breadcrumbs)
    TopBottomPanel::top("title_panel")
        .default_height(56.)
        .resizable(false)
        .show(ctx, |ui| {
            components::title::render_title_ui(ui, inner);
        });

    // Central area: delegate to per-page renderers
    pages::render_panels(ctx, inner, bus, frame);

    // Bottom status panel
    TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
        status::render_status_ui(ui);
    });
}
