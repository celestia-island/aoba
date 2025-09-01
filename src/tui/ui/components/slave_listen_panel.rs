use ratatui::{prelude::*, text::Line};

use crate::{i18n::lang, protocol::status::Status, tui::ui::components::render_boxed_paragraph};

/// Render a slave listen panel (distinct from registers_panel) — uses i18n strings.
pub fn render_slave_listen_panel(f: &mut Frame, area: Rect, _app: &mut Status) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(lang().label_slave_listen.as_str()));
    lines.push(Line::from(lang().details_placeholder.as_str()));
    render_boxed_paragraph(f, area, lines, None);
}
