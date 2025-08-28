use ratatui::prelude::*;

pub mod details;
pub mod ports;

use crate::protocol::status::Status;

pub fn render_panels(f: &mut Frame, area: Rect, app: &Status) {
    // main area split horizontally into left/right panels (bottom bar handled by bottom::render_bottom)
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(60),
        ])
        .split(area);

    ports::render_ports(f, chunks[0], app);
    details::render_details(f, chunks[1], app);
}
