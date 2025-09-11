mod bottom;
mod components;
pub mod pages;
mod title;

use ratatui::prelude::*;

use crate::protocol::status::ui as ui_accessors;
use crate::protocol::status::Status;
use crate::tui::ui::components::mode_selector::render_mode_selector;

pub fn render_ui(f: &mut Frame, app: &mut Status) {
    let area = f.area();
    let bottom_len =
        if ui_accessors::ui_error_get(app).is_some() || ui_accessors::ui_subpage_active_get(app) {
            2
        } else {
            1
        };
    let main_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Length(1), // title
            ratatui::layout::Constraint::Min(0),    // main
            ratatui::layout::Constraint::Length(bottom_len),
        ])
        .split(area);

    title::render_title(f, main_chunks[0], app);
    pages::render_panels(f, main_chunks[1], app);
    bottom::render_bottom(f, main_chunks[2], app);

    if ui_accessors::ui_mode_overlay_active_get(app) {
        let idx = ui_accessors::ui_mode_overlay_index_get(app)
            .as_usize()
            .min(1);
        render_mode_selector(f, idx);
    }
}
