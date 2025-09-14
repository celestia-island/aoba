use ratatui::{prelude::*, widgets::*};

use crate::{
    i18n::lang,
    protocol::status::types::{self, port::PortData, Status},
};

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
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    let title_text = if subpage_active {
        // derive selection from page
        let sel = match &app.page {
            types::Page::Entry { cursor } => match cursor {
                Some(types::ui::EntryCursor::Com { idx }) => *idx,
                Some(types::ui::EntryCursor::About) => app.ports.order.len().saturating_add(2),
                Some(types::ui::EntryCursor::Refresh) => app.ports.order.len(),
                Some(types::ui::EntryCursor::CreateVirtual) => {
                    app.ports.order.len().saturating_add(1)
                }
                None => 0usize,
            },
            types::Page::ModbusDashboard { selected_port, .. }
            | types::Page::ModbusConfig { selected_port, .. }
            | types::Page::ModbusLog { selected_port, .. } => *selected_port,
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
