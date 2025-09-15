use std::cmp::min;

use ratatui::{prelude::*, text::Line};

use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
    tui::ui::components::render_boxed_paragraph,
    // Bus intentionally unused in render; kept for parity with original file
};

/// Render the ModBus panel. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status, _snap: &types::ui::ModbusDashboardStatus) {
    let mut lines: Vec<Line> = Vec::new();

    // Simple display of ModBus status
    lines.push(Line::from("ModBus Panel"));
    lines.push(Line::from(""));

    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. }
    );
    if subpage_active {
        lines.push(Line::from(
            "Subpage form present (details moved to UI layer)",
        ));
    } else {
        lines.push(Line::from("No form data available"));
    }

    // Calculate visible area for scrolling
    let inner_height = area.height.saturating_sub(2) as usize;
    // Core no longer stores SubpageForm; default cursor to 0 for rendering purposes.
    let cursor_line = 0;

    let mut first_visible = 0;
    if cursor_line >= inner_height {
        first_visible = cursor_line + 1 - inner_height;
    }

    let total = lines.len();
    let last_start = total.saturating_sub(inner_height);
    if first_visible > last_start {
        first_visible = last_start;
    }
    let end = min(total, first_visible + inner_height);

    render_boxed_paragraph(f, area, lines[first_visible..end].to_vec(), None);
}

pub fn page_bottom_hints(_app: &Status, _snap: &types::ui::ModbusDashboardStatus) -> Vec<String> {
    let hints: Vec<String> = vec![
        lang().hotkeys.hint_move_vertical.as_str().to_string(),
        "Enter: Edit".to_string(),
        "Del: Delete".to_string(),
    ];
    hints
}
