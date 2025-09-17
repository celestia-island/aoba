pub mod about;
pub mod config_panel;
pub mod entry;
pub mod log_panel;
pub mod modbus_panel;

use ratatui::prelude::*;

use crate::protocol::status::types::{self, Status};

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
/// This function only reads from Status and renders - no mutations allowed.
pub fn render_panels(f: &mut Frame, area: Rect, app: &Status) {
    // Dispatch directly on the current page variant (About rendered here directly)
    match &app.page {
        types::Page::ModbusConfig { .. } => {
            let _ = config_panel::render(f, area, None);
        }
        types::Page::ModbusDashboard { .. } => {
            let _ = modbus_panel::render(f, area);
        }
        types::Page::ModbusLog { .. } => {
            let _ = log_panel::render(f, area);
        }
        types::Page::About { .. } => {
            let _ = about::render(f, area);
        }
        types::Page::Entry { .. } => {
            let _ = entry::render(f, area);
        }
    }
}
