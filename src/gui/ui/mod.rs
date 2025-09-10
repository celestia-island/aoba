pub mod components;
pub mod pages;
pub mod status;
pub mod title;

use crate::protocol::status::Status;
use crate::tui::utils::bus::Bus;
use eframe::egui::Context;
use eframe::Frame;
use egui::TopBottomPanel;

/// Centralized UI renderer: title, breadcrumb navigation, pages and bottom status.
pub fn render_ui(
    ctx: &Context,
    frame: &mut Frame,
    inner: &std::sync::Arc<std::sync::Mutex<Status>>,
    bus: &Bus,
) {
    // Top panel (title + breadcrumbs)
    TopBottomPanel::top("title_panel")
        .default_height(56.0)
        .resizable(false)
        .show(ctx, |ui| {
            title::render_title_ui(ui, inner);
        });

    // Central area: delegate to per-page renderers
    pages::render_panels(ctx, inner, bus, frame);

    // Bottom status panel
    TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
        status::render_status_ui(ui);
    });
}
