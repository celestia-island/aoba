use std::sync::{Arc, RwLock};

use eframe::{egui::Context, Frame};

use crate::{protocol::status::Status, tui::utils::bus::Bus};

/// Render top-level panels by delegating to per-page renderers.
pub fn render_panels(_ctx: &Context, _inner: &Arc<RwLock<Status>>, _bus: &Bus, _frame: &mut Frame) {
    // TODO: implement GUI page rendering
}
