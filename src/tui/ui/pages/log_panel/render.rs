use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::read_status,
    tui::ui::pages::log_panel::components::{
        extract_log_data, render_log_display, render_log_input,
    },
};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    // Check if we're in free view mode to conditionally show the follow hint
    let show_follow_hint = read_status(|status| {
        if let crate::protocol::status::types::Page::LogPanel { selected_item, .. } = &status.page {
            Ok(selected_item.is_some()) // Show hint only when in manual mode (Some)
        } else {
            Ok(false)
        }
    })
    .unwrap_or(false);

    let mut hints = vec![vec![
        lang().hotkeys.hint_move_vertical.as_str().to_string(),
        lang().hotkeys.press_c_clear.as_str().to_string(),
    ]];

    let mut second_row = vec![lang().hotkeys.press_esc_cancel.as_str().to_string()];

    if show_follow_hint {
        second_row.insert(0, "v=follow latest logs".to_string());
    }

    hints.push(second_row);
    Ok(hints)
}

/// Render the log panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    // Split the area: upper area for logs, lower 3 lines for input
    let chunks: [Rect; 2] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Min(3),
        ratatui::layout::Constraint::Length(3),
    ])
    .areas(area);

    let logs_area = chunks[0];
    let input_area = chunks[1];

    if let Some((logs, port_log_auto_scroll, selected_item)) = extract_log_data()? {
        let _ = render_log_display(frame, logs_area, &logs, port_log_auto_scroll, selected_item);
    }

    render_log_input(frame, input_area)?;

    Ok(())
}
