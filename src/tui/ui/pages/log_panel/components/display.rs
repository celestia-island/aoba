use anyhow::Result;

use ratatui::{
    prelude::*,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    i18n::lang,
    protocol::status::{
        read_status,
        types,
        with_port_read,
    },
};

/// Extract log data from current page state
pub fn extract_log_data() -> Result<Option<(Vec<types::port::PortLogEntry>, bool)>> {
    let res = read_status(|status| match &status.page {
        types::Page::LogPanel { selected_port, .. } => {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Some(tuple) =
                        with_port_read(port, |pd| Some((pd.logs.clone(), pd.log_auto_scroll)))
                    {
                        Ok(tuple)
                    } else {
                        log::warn!("extract_log_data: failed to acquire read lock for {port_name}");
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    })?;
    Ok(res)
}

/// Render the main log display area
pub fn render_log_display(
    frame: &mut Frame,
    area: Rect,
    logs: &[types::port::PortLogEntry],
    view_offset: usize,
    follow_active: bool,
) -> Result<()> {
    // Simplified rendering logic
    let lines: Vec<Line> = logs
        .iter()
        .skip(view_offset)
        .take(area.height as usize)
        .map(|entry| {
            Line::from(vec![
                Span::styled(
                    entry.when.format("%H:%M:%S%.3f").to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(" "),
                Span::raw(entry.raw.clone()),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " {} {} ",
            lang().protocol.common.log_monitoring.clone(),
            if follow_active { "[F]" } else { "" }
        ));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);

    Ok(())
}

/// Render the log input area
pub fn render_log_input(frame: &mut Frame, area: Rect) -> Result<()> {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", lang().input.input_label.clone()));

    let content = vec![Line::from(format!(
        "{} | {} | {}", 
        lang().hotkeys.press_enter_toggle.clone(),
        lang().hotkeys.press_c_clear.clone(),
        lang().hotkeys.press_esc_cancel.clone()
    ))];

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);

    Ok(())
}