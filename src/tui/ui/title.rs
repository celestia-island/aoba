use anyhow::Result;

use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read},
};

fn get_port_name(selected_port: usize) -> Result<String> {
    let port_name = if selected_port < read_status(|status| Ok(status.ports.order.len()))? {
        read_status(|status| {
            let name = status.ports.order[selected_port].clone();
            Ok(status
                .ports
                .map
                .get(&name)
                .and_then(|port| with_port_read(port, |port| port.port_name.clone()))
                .unwrap_or_else(|| format!("COM{selected_port}")))
        })?
    } else {
        format!("COM{selected_port}")
    };
    Ok(port_name)
}

pub fn render_title(frame: &mut Frame, area: Rect) -> Result<()> {
    // Horizontal layout: left (spinner + breadcrumb) + right (reserved)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(2)])
        .split(area);

    // Background bar
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray));
    frame.render_widget(bg_block, area);

    // Build breadcrumb as a sequence of styled Spans with spinner at the beginning
    let mut breadcrumb_spans: Vec<Span> = Vec::new();

    // Always reserve 2 spaces from left then draw spinner which always animates.
    // Spinner color: yellow when busy, white when idle.
    let busy = read_status(|status| Ok(status.temporarily.busy.busy))?;
    let frame_index = read_status(|status| Ok(status.temporarily.busy.spinner_frame))? as usize;
    let frames = ['⠏', '⠛', '⠹', '⠼', '⠶', '⠧'];
    let ch = frames[frame_index % frames.len()];
    // leading spaces
    breadcrumb_spans.push(Span::raw("  "));
    // spinner with color depending on busy
    let spinner_style = if busy {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    breadcrumb_spans.push(Span::styled(ch.to_string(), spinner_style));
    breadcrumb_spans.push(Span::raw("   "));

    // Add breadcrumb path based on current page
    let page_breadcrumb = match read_status(|status| Ok(status.page.clone()))? {
        // Entry page: AOBA title
        types::Page::Entry { .. } => lang().index.title.as_str().to_string(),

        // Port configuration page: AOBA title > COMx
        types::Page::ConfigPanel { selected_port, .. } => {
            let port_name = get_port_name(selected_port)?;
            format!("{} > {}", lang().index.title.as_str(), port_name)
        }

        // Modbus master/slave configuration: AOBA title > COMx > Modbus
        types::Page::ModbusDashboard { selected_port, .. } => {
            let port_name = get_port_name(selected_port)?;
            format!(
                "{} > {} > {}",
                lang().index.title.as_str(),
                port_name,
                lang().protocol.modbus.label_modbus_settings.as_str()
            )
        }

        // Manual debug log: AOBA title > COMx > Communication Log
        types::Page::LogPanel { selected_port, .. } => {
            let port_name = get_port_name(selected_port)?;
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

    // Append breadcrumb text as a styled span (light green, bold)
    breadcrumb_spans.push(Span::styled(
        page_breadcrumb,
        Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD),
    ));

    let title_para = Paragraph::new(vec![Line::from(breadcrumb_spans)]).alignment(Alignment::Left);
    frame.render_widget(title_para, chunks[0]);

    Ok(())
}
