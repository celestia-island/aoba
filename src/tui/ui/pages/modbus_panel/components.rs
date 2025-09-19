use std::cmp::min;

use ratatui::{prelude::*, text::Line};

use crate::{
    i18n::lang,
    protocol::status::{types, write_status},
    tui::ui::components::boxed_paragraph::render_boxed_paragraph,
};

/// Check if a subpage is currently active for modbus panel
pub fn is_subpage_active() -> bool {
    // Read status and determine subpage active
    if let Ok(active) = crate::protocol::status::read_status(|app| {
        Ok(matches!(
            app.page,
            types::Page::ConfigPanel { .. } | types::Page::ModbusDashboard { .. }
        ))
    }) {
        active
    } else {
        false
    }
}

/// Generate status lines for modbus panel display
pub fn generate_modbus_status_lines() -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Simple display of ModBus status
    lines.push(Line::from("ModBus Panel"));
    lines.push(Line::from(""));

    if is_subpage_active() {
        lines.push(Line::from(
            "Subpage form present (details moved to UI layer)",
        ));
    } else {
        lines.push(Line::from("No form data available"));
    }

    lines
}

/// Calculate scrolling parameters for the modbus panel
pub fn calculate_scroll_params(lines: &[Line], area: Rect, cursor_line: usize) -> (usize, usize) {
    // Calculate visible area for scrolling
    let inner_height = area.height.saturating_sub(2) as usize;

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

    (first_visible, end)
}

/// Render the modbus panel content with scrolling
pub fn render_modbus_content(frame: &mut Frame, area: Rect, lines: Vec<Line>, view_offset: usize) {
    // Use the view_offset from page state instead of calculating scroll params
    render_boxed_paragraph(frame, area, lines, view_offset, None, false, true);
}

/// Get bottom hints for modbus panel
pub fn get_modbus_bottom_hints() -> Vec<Vec<String>> {
    vec![
        vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
        vec!["Enter: Edit".to_string(), "Del: Delete".to_string()],
    ]
}

/// Scroll the ModbusDashboard view offset up by `amount` (saturating at 0).
pub fn modbus_panel_scroll_up(amount: usize) -> anyhow::Result<()> {
    write_status(|s| {
        if let types::Page::ModbusDashboard { view_offset, .. } = &mut s.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the ModbusDashboard view offset down by `amount`.
pub fn modbus_panel_scroll_down(amount: usize) -> anyhow::Result<()> {
    write_status(|s| {
        if let types::Page::ModbusDashboard { view_offset, .. } = &mut s.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}
