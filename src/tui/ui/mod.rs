pub(self) mod bottom;
pub(self) mod components;
pub mod pages;
pub(self) mod title;

use ratatui::prelude::*;

use crate::protocol::status::Status;

pub fn render_ui(f: &mut Frame, app: &mut Status) {
    let area = f.area();
    // Reduce outer margin so middle panel is closer to title and bottom bar.
    // Reserve 2 lines for bottom when there's an error or when a subpage is active
    let bottom_len = if app.error.is_some() || app.active_subpage.is_some() {
        2
    } else {
        1
    };
    let main_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Length(1), // Title
            ratatui::layout::Constraint::Min(0),
            ratatui::layout::Constraint::Length(bottom_len), // Bottom help (1 or 2 lines)
        ])
        .split(area);

    // Render subcomponents
    title::render_title(f, main_chunks[0], app);
    pages::render_panels(f, main_chunks[1], app);
    bottom::render_bottom(f, main_chunks[2], app);

    // If a global mode selector is active, render it last so it appears above everything
    if app.mode_selector_active {
        crate::tui::ui::components::mode_selector::render_mode_selector(f, app.mode_selector_index);
    }
}
