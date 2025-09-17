use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::ui::pages::entry::components::{
        derive_selection_from_page, render_details_panel, render_ports_list,
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    read_status(|app| {
        let in_subpage_editing = false;
        let subpage_active = matches!(
            app.page,
            types::Page::ModbusConfig { .. }
                | types::Page::ModbusDashboard { .. }
                | types::Page::ModbusLog { .. }
                | types::Page::About { .. }
        );

        let mut base = vec![
            vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
            vec![lang().hotkeys.hint_enter_subpage.as_str().to_string()],
        ];
        if !subpage_active && !in_subpage_editing {
            base.push(vec![lang().hotkeys.press_q_quit.as_str().to_string()]);
        }
        Ok(base)
    })
    .unwrap_or_else(|_| vec![])
}

/// Render the entry page. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    // Horizontal split: left ports | right details
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(60),
        ])
        .split(area);

    let left = chunks[0];
    let right = chunks[1];

    read_status(|app| {
        // Derive current selection index from page
        let selection = derive_selection_from_page(&app.page, &app.ports.order);

        // LEFT: ports list
        render_ports_list(frame, left, app, selection);

        // RIGHT: content details
        render_details_panel(frame, right, app, selection);

        Ok(())
    })?;

    Ok(())
}
