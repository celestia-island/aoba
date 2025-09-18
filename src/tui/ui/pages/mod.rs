pub mod about;
pub mod config_panel;
pub mod entry;
pub mod log_panel;
pub mod modbus_panel;

use anyhow::Result;

use ratatui::prelude::*;

use crate::protocol::status::{
    read_status,
    types::{self, Status},
};

/// Return page-provided bottom hints for the current app state.
/// Now returns a Vec of rows, where each row is a Vec of hint fragments.
pub fn bottom_hints_for_app(app: &Status) -> Vec<Vec<String>> {
    // Dispatch directly on the current page variant (About handled here directly)
    match &app.page {
        types::Page::ModbusConfig { .. } => config_panel::page_bottom_hints(),
        types::Page::ModbusDashboard { .. } => modbus_panel::page_bottom_hints(),
        types::Page::ModbusLog { .. } => log_panel::page_bottom_hints(),
        types::Page::About { .. } => about::page_bottom_hints(),
        types::Page::Entry { .. } => entry::page_bottom_hints(),
    }
}

/// Render the appropriate page based on the current app state.
pub fn render_panels(frame: &mut Frame, area: Rect) -> Result<()> {
    match read_status(|app| Ok(app.page.clone()))? {
        types::Page::Entry { .. } => {
            entry::render(frame, area)?;
        }
        types::Page::About { .. } => {
            about::render(frame, area)?;
        }
        types::Page::ModbusConfig { .. } => {
            config_panel::render(frame, area)?;
        }
        types::Page::ModbusDashboard { .. } => {
            modbus_panel::render(frame, area)?;
        }
        types::Page::ModbusLog { .. } => {
            log_panel::render(frame, area)?;
        }
    }

    Ok(())
}
