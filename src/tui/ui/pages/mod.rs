pub mod about;
pub mod config_panel;
pub mod entry;
pub mod log_panel;
pub mod modbus_panel;

use anyhow::Result;

use ratatui::prelude::*;

use crate::tui::status::read_status;

/// Return page-provided bottom hints for the current app state.
/// Now returns a Vec of rows, where each row is a Vec of hint fragments.
pub fn bottom_hints_for_app() -> Result<Vec<Vec<String>>> {
    Ok(match &read_status(|status| Ok(status.page.clone()))? {
        crate::tui::status::Page::ConfigPanel { .. } => config_panel::page_bottom_hints()?,
        crate::tui::status::Page::ModbusDashboard { .. } => modbus_panel::page_bottom_hints()?,
        crate::tui::status::Page::LogPanel { .. } => log_panel::page_bottom_hints()?,
        crate::tui::status::Page::About { .. } => about::page_bottom_hints()?,
        crate::tui::status::Page::Entry { .. } => entry::page_bottom_hints()?,
    })
}

/// Render the appropriate page based on the current app state.
pub fn render_panels(frame: &mut Frame, area: Rect) -> Result<()> {
    match read_status(|app| Ok(app.page.clone()))? {
        crate::tui::status::Page::Entry { .. } => {
            entry::render(frame, area)?;
        }
        crate::tui::status::Page::About { .. } => {
            about::render(frame, area)?;
        }
        crate::tui::status::Page::ConfigPanel { .. } => {
            config_panel::render(frame, area)?;
        }
        crate::tui::status::Page::ModbusDashboard { .. } => {
            modbus_panel::render(frame, area)?;
        }
        crate::tui::status::Page::LogPanel { .. } => {
            log_panel::render(frame, area)?;
        }
    }

    Ok(())
}
