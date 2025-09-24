use ratatui::{prelude::*, text::Line};

use crate::{
    protocol::status::{read_status, types},
    tui::ui::components::boxed_paragraph::render_boxed_paragraph,
};

/// Generate status lines for modbus panel display
pub fn generate_modbus_status_lines() -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    lines
}

/// Render the modbus panel content with scrolling
pub fn render_modbus_content(frame: &mut Frame, area: Rect, lines: Vec<Line>, view_offset: usize) {
    // Use the view_offset from page state instead of calculating scroll params
    render_boxed_paragraph(frame, area, lines, view_offset, None, false, true);
}
