pub mod bottom;
pub mod components;
pub mod pages;
pub mod title;

use ratatui::prelude::*;

use crate::protocol::status::types::Status;
use crate::tui::ui::components::mode_selector::render_mode_selector;

pub fn render_ui(f: &mut Frame, app: &mut Status) {
    let area = f.area();
    let subpage_active = matches!(
        app.page,
        crate::protocol::status::types::Page::ModbusConfig { .. }
            | crate::protocol::status::types::Page::ModbusDashboard { .. }
            | crate::protocol::status::types::Page::ModbusLog { .. }
            | crate::protocol::status::types::Page::About { .. }
    );
    let bottom_len = if app.temporarily.error.is_some() || subpage_active {
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

    if app.temporarily.modals.mode_selector.active {
        let idx = app
            .temporarily
            .modals
            .mode_selector
            .selector
            .as_usize()
            .min(1);
        render_mode_selector(f, idx);
    }
}
