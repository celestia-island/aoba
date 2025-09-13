use ratatui::prelude::*;

use crate::{
    i18n::lang, protocol::status::types::Status, tui::ui::components::render_boxed_paragraph,
    tui::utils::bus::Bus,
};

/// Render a configuration panel for a subpage. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status, style: Option<Style>) {
    // Consider the subpage active if `page` is one of the Modbus subpages.
    let subpage_active = matches!(app.page, crate::protocol::status::types::Page::ModbusConfig { .. } | crate::protocol::status::types::Page::ModbusDashboard { .. });
    if !subpage_active {
        let lines = vec![ratatui::text::Line::from("No form loaded")];
        return render_boxed_paragraph(f, area, lines, style);
    }

    // If the UI needs per-field state it should derive it from Status or local state.
    // For now render a simplified placeholder view.

    // Since `SubpageForm` was removed from core status, render a simplified placeholder
    // that displays high-level information derived from `Status` or instructs the UI
    // to manage detailed editing state locally.
    let lines = vec![ratatui::text::Line::from(
        "Configuration panel: details managed in UI layer",
    )];
    render_boxed_paragraph(f, area, lines, style);
}

pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    let hints: Vec<String> = vec![
        lang().hotkeys.hint_move_vertical.as_str().to_string(),
        "Enter: Edit".to_string(),
    ];
    hints
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
}

/// Handle input for config panel. Sends commands via UiToCore.
pub fn handle_input(_key: crossterm::event::KeyEvent, bus: &Bus) -> bool {
    use crossterm::event::KeyCode as KC;

    match _key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Navigation
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Enter => {
            // Edit field
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Esc => {
            // Cancel edit
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false,
    }
}
