use ratatui::{prelude::*, widgets::*};

use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
};

pub fn render_title(f: &mut Frame, area: Rect, app: &mut Status) {
    render_title_readonly(f, area, app);
}

pub fn render_title_readonly(f: &mut Frame, area: Rect, app: &Status) {
    // Horizontal layout: left (spinner + breadcrumb) + right (reserved)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    // Background bar
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray));
    f.render_widget(bg_block, area);

    // Build breadcrumb text with spinner at the beginning
    let mut breadcrumb_text = String::new();
    
    // Add spinner if busy (2 spaces from left)
    breadcrumb_text.push_str("  ");
    if app.temporarily.busy.busy {
        let frames = ['◜', '◝', '◞', '◟'];
        let ch = frames[(app.temporarily.busy.spinner_frame as usize) % frames.len()];
        breadcrumb_text.push(ch);
        breadcrumb_text.push(' ');
    }

    // Add breadcrumb path based on current page
    let page_breadcrumb = match &app.page {
        // Entry page: AOBA title
        types::Page::Entry { .. } => lang().index.title.as_str().to_string(),

        // Port configuration page: AOBA title > COMx
        types::Page::ModbusConfig { selected_port, .. } => {
            let port_name = if *selected_port < app.ports.order.len() {
                let name = &app.ports.order[*selected_port];
                app.ports
                    .map
                    .get(name)
                    .map(|p| p.port_name.clone())
                    .unwrap_or_else(|| format!("COM{}", selected_port))
            } else {
                format!("COM{}", selected_port)
            };
            format!("{} > {}", lang().index.title.as_str(), port_name)
        }

        // Modbus master/slave configuration: AOBA title > COMx > Modbus
        types::Page::ModbusDashboard { selected_port, .. } => {
            let port_name = if *selected_port < app.ports.order.len() {
                let name = &app.ports.order[*selected_port];
                app.ports
                    .map
                    .get(name)
                    .map(|p| p.port_name.clone())
                    .unwrap_or_else(|| format!("COM{}", selected_port))
            } else {
                format!("COM{}", selected_port)
            };
            format!(
                "{} > {} > {}",
                lang().index.title.as_str(),
                port_name,
                lang().protocol.modbus.label_modbus_settings.as_str()
            )
        }

        // Manual debug log: AOBA title > COMx > Communication Log
        types::Page::ModbusLog { selected_port, .. } => {
            let port_name = if *selected_port < app.ports.order.len() {
                let name = &app.ports.order[*selected_port];
                app.ports
                    .map
                    .get(name)
                    .map(|p| p.port_name.clone())
                    .unwrap_or_else(|| format!("COM{}", selected_port))
            } else {
                format!("COM{}", selected_port)
            };
            format!(
                "{} > {} > {}",
                lang().index.title.as_str(),
                port_name,
                lang().tabs.tab_log.as_str()
            )
        }

        // About page: AOBA title > About
        types::Page::About { .. } => {
            format!(
                "{} > {}",
                lang().index.title.as_str(),
                lang().index.about_label.as_str()
            )
        }
    };
    
    breadcrumb_text.push_str(&page_breadcrumb);

    let title_para = Paragraph::new(breadcrumb_text)
        .alignment(Alignment::Left)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(title_para, chunks[0]);
}
