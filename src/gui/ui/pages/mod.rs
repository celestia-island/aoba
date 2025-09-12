use std::sync::{Arc, RwLock};

use eframe::{egui::Context, Frame};

use crate::{protocol::status::Status, tui::utils::bus::Bus};

/// Render top-level panels by delegating to per-page renderers.
pub fn render_panels(ctx: &Context, inner: &Arc<RwLock<Status>>, bus: &Bus, frame: &mut Frame) {
    // TODO: implement GUI page rendering
}
