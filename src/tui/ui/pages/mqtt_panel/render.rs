use ratatui::{prelude::*, text::Line};

use crate::{
    protocol::status::types::{self, Status},
    tui::{ui::components::render_boxed_paragraph},
};

/// Render the MQTT panel. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status, _snap: &types::ui::EntryStatus) {
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
        types::Page::Entry { cursor } => match cursor {
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            Some(types::ui::EntryCursor::About) => app.ports.order.len().saturating_add(2),
            Some(types::ui::EntryCursor::Refresh) => app.ports.order.len(),
            Some(types::ui::EntryCursor::CreateVirtual) => app.ports.order.len().saturating_add(1),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        _ => 0usize,
    };
    lines.push(Line::from(format!("Selected port: {sel}")));
    // MQTT panel placeholder.

    render_boxed_paragraph(f, area, lines, None);
}

pub fn page_bottom_hints(_app: &Status, _snap: &types::ui::EntryStatus) -> Vec<String> {
    let hints: Vec<String> = vec!["MQTT not implemented".to_string()];
    hints
}
