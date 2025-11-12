use anyhow::Result;

use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::{
    protocol::status::types::port::PortStatusIndicator, tui::status::read_status, utils::i18n::lang,
};

fn get_port_name(selected_port: usize) -> Result<String> {
    let port_name = if selected_port < read_status(|status| Ok(status.ports.order.len()))? {
        read_status(|status| {
            let name = status.ports.order[selected_port].clone();
            Ok(status
                .ports
                .map
                .get(&name)
                .map(|port| port.port_name.clone())
                .unwrap_or_else(|| format!("COM{selected_port}")))
        })?
    } else {
        format!("COM{selected_port}")
    };
    Ok(port_name)
}

/// Get the status indicator for the currently selected port
fn get_port_status_indicator(selected_port: usize) -> Result<Option<PortStatusIndicator>> {
    read_status(|status| {
        let port_name_opt = status.ports.order.get(selected_port).cloned();
        if let Some(port_name) = port_name_opt {
            if let Some(port) = status.ports.map.get(&port_name) {
                return Ok(Some(port.status_indicator.clone()));
            }
        }
        Ok(None)
    })
}

pub fn render_title(frame: &mut Frame, area: Rect) -> Result<()> {
    // Horizontal layout: left (icon + breadcrumb) + right (status indicator)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(30)])
        .split(area);

    // Background bar
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray));
    frame.render_widget(bg_block, area);

    // Build breadcrumb with port icon on the left
    let mut breadcrumb_spans: Vec<Span> = Vec::new();

    // Static port/plug icon on the left (🔌 plug emoji)
    breadcrumb_spans.push(Span::raw("  "));
    breadcrumb_spans.push(Span::styled("🔌", Style::default().fg(Color::White)));
    breadcrumb_spans.push(Span::raw("   "));

    // Add breadcrumb path based on current page
    let page_breadcrumb = match read_status(|status| Ok(status.page.clone()))? {
        // Entry page: AOBA title
        crate::tui::status::Page::Entry { .. } => lang().index.title.as_str().to_string(),

        // Port configuration page: AOBA title > COMx
        crate::tui::status::Page::ConfigPanel { selected_port, .. } => {
            let port_name = get_port_name(selected_port)?;
            format!("{} > {}", lang().index.title.as_str(), port_name)
        }

        // Modbus master/slave configuration: AOBA title > COMx > Modbus
        crate::tui::status::Page::ModbusDashboard { selected_port, .. } => {
            let port_name = get_port_name(selected_port)?;
            format!(
                "{} > {} > {}",
                lang().index.title.as_str(),
                port_name,
                lang().protocol.modbus.label_modbus_settings.as_str()
            )
        }

        // Manual debug log: AOBA title > COMx > Communication Log
        crate::tui::status::Page::LogPanel { selected_port, .. } => {
            let port_name = get_port_name(selected_port)?;
            format!(
                "{} > {} > {}",
                lang().index.title.as_str(),
                port_name,
                lang().tabs.tab_log.as_str()
            )
        }

        // About page: AOBA title > About
        crate::tui::status::Page::About { .. } => {
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

    // Render status indicator on the right side (only for port-related pages)
    let selected_port_opt = match read_status(|status| Ok(status.page.clone()))? {
        crate::tui::status::Page::ConfigPanel { selected_port, .. }
        | crate::tui::status::Page::ModbusDashboard { selected_port, .. }
        | crate::tui::status::Page::LogPanel { selected_port, .. } => Some(selected_port),
        _ => None,
    };

    if let Some(selected_port) = selected_port_opt {
        if let Some(indicator) = get_port_status_indicator(selected_port)? {
            let (status_text, status_icon, status_color) = get_status_display(&indicator)?;

            // Check if we need to show time-limited statuses
            let should_show_status = match indicator {
                PortStatusIndicator::AppliedSuccess { timestamp } => {
                    use chrono::Local;
                    let elapsed = Local::now().signed_duration_since(timestamp);
                    elapsed.num_seconds() < 3
                }
                PortStatusIndicator::StartupFailed { timestamp, .. } => {
                    // Show startup failure for 10 seconds
                    use chrono::Local;
                    let elapsed = Local::now().signed_duration_since(timestamp);
                    elapsed.num_seconds() < 10
                }
                _ => true,
            };

            if should_show_status {
                let status_spans = vec![
                    Span::styled(status_text, Style::default().fg(status_color)),
                    Span::raw(" "),
                    Span::styled(status_icon, Style::default().fg(status_color)),
                    Span::raw(" "), // Add trailing space before terminal edge
                ];

                let status_para =
                    Paragraph::new(vec![Line::from(status_spans)]).alignment(Alignment::Right);
                frame.render_widget(status_para, chunks[1]);
            }
        }
    }

    Ok(())
}

/// Get the display text, icon, and color for a port status indicator
fn get_status_display(indicator: &PortStatusIndicator) -> Result<(String, String, Color)> {
    let lang = lang();

    match indicator {
        PortStatusIndicator::NotStarted => Ok((
            lang.protocol.common.status_not_started.clone(),
            "×".to_string(),
            Color::Red,
        )),
        PortStatusIndicator::Starting => {
            // Spinning animation for "starting" state
            let frame_index =
                read_status(|status| Ok(status.temporarily.busy.spinner_frame))? as usize;
            let frames = ['⠏', '⠛', '⠹', '⠼', '⠶', '⠧'];
            let spinner = frames[frame_index % frames.len()].to_string();

            Ok((
                lang.protocol.common.status_starting.clone(),
                spinner,
                Color::Yellow,
            ))
        }
        PortStatusIndicator::Running => Ok((
            lang.protocol.common.status_running.clone(),
            "●".to_string(),
            Color::Green,
        )),
        PortStatusIndicator::Restarting => {
            // Yellow spinning animation for "restarting" state
            let frame_index =
                read_status(|status| Ok(status.temporarily.busy.spinner_frame))? as usize;
            let frames = ['⠏', '⠛', '⠹', '⠼', '⠶', '⠧'];
            let spinner = frames[frame_index % frames.len()].to_string();

            Ok((
                lang.protocol.common.status_restarting.clone(),
                spinner,
                Color::Yellow,
            ))
        }
        PortStatusIndicator::Saving => {
            // Green spinning animation for "saving" state
            let frame_index =
                read_status(|status| Ok(status.temporarily.busy.spinner_frame))? as usize;
            let frames = ['⠏', '⠛', '⠹', '⠼', '⠶', '⠧'];
            let spinner = frames[frame_index % frames.len()].to_string();

            Ok((
                lang.protocol.common.status_saving.clone(),
                spinner,
                Color::Green,
            ))
        }
        PortStatusIndicator::Syncing => {
            // Yellow spinning animation for "syncing" state
            let frame_index =
                read_status(|status| Ok(status.temporarily.busy.spinner_frame))? as usize;
            let frames = ['⠏', '⠛', '⠹', '⠼', '⠶', '⠧'];
            let spinner = frames[frame_index % frames.len()].to_string();

            Ok((
                lang.protocol.common.status_syncing.clone(),
                spinner,
                Color::Yellow,
            ))
        }
        PortStatusIndicator::AppliedSuccess { .. } => Ok((
            lang.protocol.common.status_applied_success.clone(),
            "✔".to_string(),
            Color::Green,
        )),
        PortStatusIndicator::StartupFailed { error_message, .. } => {
            // Show truncated error message in red
            let truncated_msg = if error_message.len() > 30 {
                format!("{}...", &error_message[..27])
            } else {
                error_message.clone()
            };
            Ok((
                format!(
                    "{}: {}",
                    lang.protocol.common.status_startup_failed, truncated_msg
                ),
                "✘".to_string(),
                Color::Red,
            ))
        }
    }
}
