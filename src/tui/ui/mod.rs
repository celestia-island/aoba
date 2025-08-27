pub(self) mod bottom;
pub(self) mod panels;
pub(self) mod title;

use ratatui::prelude::*;

use crate::tui::app::App;

pub fn render_ui(f: &mut Frame, app: &App) {
    let area = f.area();
    // Reduce outer margin so middle panel is closer to title and bottom bar.
    let main_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Length(1), // title
            ratatui::layout::Constraint::Min(0),
            ratatui::layout::Constraint::Length(1), // bottom help (single line)
        ])
        .split(area);

    // render subcomponents
    title::render_title(f, main_chunks[0]);
    panels::render_panels(f, main_chunks[1], app);
    bottom::render_bottom(f, main_chunks[2], app);
}
