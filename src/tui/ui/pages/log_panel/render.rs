use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::read_status,
    tui::ui::pages::log_panel::components::{
        extract_log_data, is_in_subpage_editing, is_subpage_active, render_log_display,
        render_log_input,
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    read_status(|app| {
        let in_subpage_editing = is_in_subpage_editing();
        let subpage_active = is_subpage_active(app);

        let mut base = vec![
            vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
            vec!["f: Toggle follow".to_string(), "c: Clear logs".to_string()],
        ];
        if !subpage_active && !in_subpage_editing {
            base.push(vec![lang().hotkeys.press_q_quit.as_str().to_string()]);
        }
        Ok(base)
    })
    .unwrap_or_else(|_| vec![])
}

/// Render the log panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let chunks: [Rect; 2] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Min(3),
        ratatui::layout::Constraint::Length(3),
    ])
    .areas(area);

    let logs_area = chunks[0];

    read_status(|app| {
        // Only render when current page is ModbusLog pointing to a valid port
        if let Some((logs, port_log_selected, port_log_view_offset, port_log_auto_scroll)) =
            extract_log_data(app)
        {
            render_log_display(
                frame,
                logs_area,
                &logs,
                port_log_selected,
                port_log_view_offset,
                port_log_auto_scroll,
            );
        }
        Ok(())
    })?;

    // Bottom text input for sending - simplified placeholder
    render_log_input(frame, chunks[1]);

    Ok(())
}
