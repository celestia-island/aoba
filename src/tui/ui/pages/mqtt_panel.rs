use ratatui::{
    prelude::*,
    text::Line,
};

use crate::tui::utils::bus::Bus;
use crate::{
    protocol::status::Status,
    tui::ui::components::render_boxed_paragraph,
};

/// Render the MQTT panel. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status) {
    let mut lines: Vec<Line> = Vec::new();
    
    // Simple display of MQTT status
    lines.push(Line::from("MQTT Panel"));
    lines.push(Line::from(""));
    lines.push(Line::from("MQTT functionality not implemented yet"));
    lines.push(Line::from("This is a placeholder for MQTT protocol support"));
    
    // Show some basic app state info
    lines.push(Line::from(""));
    lines.push(Line::from(format!("Selected port: {}", app.page.selected)));
    lines.push(Line::from(format!("App mode: {:?}", app.page.app_mode)));
    
    render_boxed_paragraph(f, area, lines, None);
}

pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    hints.push("MQTT not implemented".to_string());
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
        _ => false
    }
}