use anyhow::Result;

use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::read_status,
    tui::ui::pages::entry::components::{
        derive_selection_from_page, render_details_panel, render_ports_list,
    },
};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    Ok(vec![
        vec![
            lang().hotkeys.hint_move_vertical.as_str().to_string(),
            lang().hotkeys.hint_enter_subpage.as_str().to_string(),
        ],
        vec![lang().hotkeys.press_q_quit.as_str().to_string()],
    ])
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

    // Components read status internally now; just derive selection and call them
    let selection = read_status(|app| Ok(derive_selection_from_page(&app.page, &app.ports.order)))?;

    // LEFT: ports list
    render_ports_list(frame, left, selection);

    // RIGHT: content details
    render_details_panel(frame, right);

    Ok(())
}
