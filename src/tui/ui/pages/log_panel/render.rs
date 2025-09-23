use anyhow::{anyhow, Result};
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    tui::ui::pages::log_panel::components::{
        extract_log_data, is_in_subpage_editing, is_subpage_active, render_log_display,
        render_log_input,
    },
};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    let in_subpage_editing = is_in_subpage_editing();
    let subpage_active = is_subpage_active();

    let mut base = vec![
        vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
        vec![
            lang().tabs.log.hint_follow_on.as_str().to_string(),
            lang().hotkeys.press_c_clear.as_str().to_string(),
        ],
    ];
    if !subpage_active && !in_subpage_editing {
        base.get_mut(0)
            .ok_or(anyhow!("Failed to get mutable reference to bottom hints"))?
            .push(lang().hotkeys.press_q_quit.as_str().to_string());
    }
    Ok(base)
}

/// Render the log panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let chunks: [Rect; 2] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Min(3),
        ratatui::layout::Constraint::Length(3),
    ])
    .areas(area);

    let logs_area = chunks[0];

    // extract_log_data / is_subpage_active read status internally
    if let Some((logs, port_log_auto_scroll)) = extract_log_data() {
        // Use page state view_offset instead of port_log_view_offset
        let _ = render_log_display(frame, logs_area, &logs, port_log_auto_scroll);
    }

    render_log_input(frame, chunks[1])?;

    Ok(())
}
