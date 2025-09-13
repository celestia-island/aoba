use crate::protocol::status::types::port::PortData;
use crate::{i18n::lang, protocol::status::types::Status};
use ratatui::{prelude::*, widgets::*};

pub fn render_title(f: &mut Frame, area: Rect, app: &mut Status) {
    render_title_readonly(f, area, app);
}

pub fn render_title_readonly(f: &mut Frame, area: Rect, app: &Status) {
    // Horizontal layout: left (spinner) + center (title) + right (reserved)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    // Background bar
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray));
    f.render_widget(bg_block, area);

    // Spinner (top-left)
    if app.temporarily.busy.busy {
        let frames = ["●○○", "○●○", "○○●"];
        let ch = frames[(app.temporarily.busy.spinner_frame as usize) % frames.len()];
        let spin = Paragraph::new(ch).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(spin, chunks[0]);
    }

    // Title text (center area)
    let subpage_active = matches!(
        app.page,
        crate::protocol::status::types::Page::ModbusConfig { .. }
            | crate::protocol::status::types::Page::ModbusDashboard { .. }
            | crate::protocol::status::types::Page::ModbusLog { .. }
            | crate::protocol::status::types::Page::About { .. }
    );
    let title_text = if subpage_active {
        // derive selection from page
        let sel = match &app.page {
            crate::protocol::status::types::Page::Entry { cursor } => match cursor {
                Some(crate::protocol::status::types::ui::EntryCursor::Com { idx }) => *idx,
                Some(crate::protocol::status::types::ui::EntryCursor::About) => {
                    app.ports.order.len().saturating_add(2)
                }
                Some(crate::protocol::status::types::ui::EntryCursor::Refresh) => {
                    app.ports.order.len()
                }
                Some(crate::protocol::status::types::ui::EntryCursor::CreateVirtual) => {
                    app.ports.order.len().saturating_add(1)
                }
                None => 0usize,
            },
            crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. }
            | crate::protocol::status::types::Page::ModbusConfig { selected_port }
            | crate::protocol::status::types::Page::ModbusLog { selected_port, .. } => {
                *selected_port
            }
            _ => 0usize,
        };
        if !app.ports.order.is_empty() && sel < app.ports.order.len() {
            let name = &app.ports.order[sel];
            let default_pd = PortData::default();
            let p = app.ports.map.get(name).unwrap_or(&default_pd);
            format!("{} - {}", p.port_name, lang().index.title.as_str())
        } else {
            lang().index.title.as_str().to_string()
        }
    } else {
        lang().index.title.as_str().to_string()
    };

    let title_para = Paragraph::new(title_text)
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(title_para, chunks[1]);
}
