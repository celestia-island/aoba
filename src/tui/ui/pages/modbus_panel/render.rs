use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    protocol::status::read_status,
    tui::ui::pages::modbus_panel::components::{
        generate_modbus_status_lines, get_modbus_bottom_hints, render_modbus_content,
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    get_modbus_bottom_hints()
}

/// Render the ModBus panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    read_status(|app| {
        let lines = generate_modbus_status_lines(app);
        render_modbus_content(frame, area, lines);
        Ok(())
    })?;

    Ok(())
}
