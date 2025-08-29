use ratatui::{prelude::*, text::Line};

use crate::{i18n::lang, protocol::status::Status};

/// Render a log panel. Uses i18n placeholder when no logs are available.
pub fn render_log_panel(f: &mut Frame, area: Rect, _app: &Status) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(lang().details_placeholder.as_str()));
    crate::tui::ui::components::render_boxed_paragraph(f, area, lines, None);
}
