use ratatui::{prelude::*, text::Line};

use crate::tui::utils::bus::Bus;
use crate::{protocol::status::types::Status, tui::ui::components::render_boxed_paragraph};

/// Render the MQTT panel. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status) {
    // Simple display of MQTT status
    let mut lines: Vec<Line> = vec![
        Line::from("MQTT Panel"),
        Line::from(""),
        Line::from("MQTT functionality not implemented yet"),
        Line::from("This is a placeholder for MQTT protocol support"),
        // Show some basic app state info
        Line::from(""),
    ];
    // derive selection from page
    let sel = match &app.page {
        crate::protocol::status::types::Page::Entry { cursor } => match cursor {
            Some(crate::protocol::status::types::ui::EntryCursor::Com { idx }) => *idx,
            Some(crate::protocol::status::types::ui::EntryCursor::About) => app.ports.order.len().saturating_add(2),
            Some(crate::protocol::status::types::ui::EntryCursor::Refresh) => app.ports.order.len(),
            Some(crate::protocol::status::types::ui::EntryCursor::CreateVirtual) => app.ports.order.len().saturating_add(1),
            None => 0usize,
        },
        crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. }
        | crate::protocol::status::types::Page::ModbusConfig { selected_port }
        | crate::protocol::status::types::Page::ModbusLog { selected_port, .. } => *selected_port,
        _ => 0usize,
    };
    lines.push(Line::from(format!("Selected port: {sel}")));
    lines.push(Line::from(format!("App mode: {:?}", app.temporarily.modals.mode_selector.selector)));

    render_boxed_paragraph(f, area, lines, None);
}

pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    let hints: Vec<String> = vec!["MQTT not implemented".to_string()];
    hints
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
}

/// Handle input for MQTT panel. Sends commands via UiToCore.
pub fn handle_input(_key: crossterm::event::KeyEvent, bus: &Bus) -> bool {
    use crossterm::event::KeyCode as KC;

    match _key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Basic navigation
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false,
    }
}
