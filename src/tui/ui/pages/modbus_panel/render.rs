use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    protocol::status::{read_status, types},
    tui::ui::pages::modbus_panel::components::{
        generate_modbus_status_lines, get_modbus_bottom_hints, render_modbus_content,
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    get_modbus_bottom_hints()
}

/// Render the ModBus panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    // Get the current view_offset from the page state
    let view_offset = read_status(|s| {
        if let types::Page::ModbusDashboard { view_offset, .. } = &s.page {
            Ok(*view_offset)
        } else {
            Ok(0)
        }
    })?;

    // generate_modbus_status_lines reads status internally
    let lines = generate_modbus_status_lines();
    render_modbus_content(frame, area, lines, view_offset);

    Ok(())
}
