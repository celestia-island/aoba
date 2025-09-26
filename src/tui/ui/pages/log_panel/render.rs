use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::ui::pages::log_panel::components::{
        extract_log_data, render_log_display, render_log_input,
    },
};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    Ok(vec![
        vec![
            lang().hotkeys.hint_move_vertical.as_str().to_string(),
            lang().hotkeys.press_c_clear.as_str().to_string(),
        ],
        vec![
            lang().tabs.log.hint_follow_on.as_str().to_string(),
            lang().hotkeys.press_esc_cancel.as_str().to_string(),
        ],
    ])
}

/// Render the log panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    // Use the entire area for logs display only (no input area)
    let logs_area = area;

    if let Some((logs, port_log_auto_scroll)) = extract_log_data()? {
        let view_offset = read_status(|status| {
            if let types::Page::LogPanel { view_offset, .. } = &status.page {
                Ok(*view_offset)
            } else {
                Ok(0)
            }
        })?;
        let _ = render_log_display(frame, logs_area, &logs, view_offset, port_log_auto_scroll);
    }

    Ok(())
}
