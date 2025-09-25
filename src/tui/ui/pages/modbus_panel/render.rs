use anyhow::Result;

use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::ui::{
        components::boxed_paragraph::render_boxed_paragraph,
        pages::modbus_panel::components::generate_modbus_status_lines,
    },
};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    Ok(vec![
        vec![
            lang().hotkeys.hint_move_vertical.as_str().to_string(),
            lang().hotkeys.hint_master_enter_edit.as_str().to_string(),
        ],
        vec![
            lang().hotkeys.hint_master_delete.as_str().to_string(),
            lang().hotkeys.hint_esc_return_home.as_str().to_string(),
        ],
    ])
}

/// Render the ModBus panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let view_offset = read_status(|status| {
        if let types::Page::ModbusDashboard { view_offset, .. } = &status.page {
            Ok(*view_offset)
        } else {
            Ok(0)
        }
    })?;

    let lines = generate_modbus_status_lines()?;
    render_boxed_paragraph(frame, area, lines, view_offset, None, false, true);

    Ok(())
}
