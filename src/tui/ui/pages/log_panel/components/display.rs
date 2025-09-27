use anyhow::Result;

use ratatui::{
    prelude::*,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read},
    tui::ui::components::boxed_paragraph::render_boxed_paragraph,
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
    _follow_active: bool, // Parameter kept for compatibility but not used
) -> Result<()> {
    // Get the actual log_auto_scroll setting from PortData
    let auto_scroll = read_status(|status| {
        if let types::Page::LogPanel { selected_port, .. } = &status.page {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Ok(port_data) = port.read() {
                        return Ok(port_data.log_auto_scroll);
                    }
                }
            }
        }
        Ok(false) // Default to false if we can't get the setting
    })
    .unwrap_or(false);

    // Each log entry is rendered as a 3-line block
    // left column: two characters reserved; when selected show a marker
    let total_lines = area.height as usize;
    let lines_per_item = 3usize;
    let items_visible = std::cmp::max(1, total_lines / lines_per_item);

    // view_offset is treated as index into log entries (item index)
    let mut rendered_lines: Vec<Line> = Vec::new();

    // Helper: parse a log entry into (line1, line2, line3) as Strings
    fn format_entry(entry: &types::port::PortLogEntry) -> (String, String, String) {
        // Line1: timestamp + status
        let time_str = entry.when.format("%H:%M:%S%.3f").to_string();

        // Determine status (very heuristic)
        let lower = entry.raw.to_lowercase();
        let status =
            if lower.contains("fail") || lower.contains("timeout") || lower.contains("error") {
                "[ERR]"
            } else {
                "[OK]"
            };

        let line1 = format!("{} {}", time_str, status);

        // Try to parse hex bytes from raw
        let mut bytes: Vec<u8> = Vec::new();
        for token in entry.raw.split_whitespace() {
            if token.len() == 2 {
                if let Ok(v) = u8::from_str_radix(token, 16) {
                    bytes.push(v);
                    continue;
                }
            }
        }

        // Build detail items
        let mut details: Vec<String> = Vec::new();
        // role and id
        if entry.raw.contains("Master") {
            if !bytes.is_empty() {
                details.push(format!("Master id={} ", bytes[0]));
            } else {
                details.push("Master".into());
            }
        } else if entry.raw.contains("Slave") {
            if !bytes.is_empty() {
                details.push(format!("Slave id={} ", bytes[0]));
            } else {
                details.push("Slave".into());
            }
        } else if !bytes.is_empty() {
            details.push(format!("id={} ", bytes[0]));
        }

        // Register type and range (best-effort)
        if bytes.len() >= 6 {
            let func = bytes[1];
            let addr = ((bytes[2] as u16) << 8) | (bytes[3] as u16);
            let qty = ((bytes[4] as u16) << 8) | (bytes[5] as u16);
            let reg_type = match func {
                0x01 => "Coils",
                0x02 => "DiscreteInputs",
                0x03 => "Holding",
                0x04 => "Input",
                0x05 => "WriteCoil",
                0x06 => "WriteHolding",
                0x0F => "WriteCoils",
                0x10 => "WriteHoldings",
                _ => "Func",
            };
            let end = addr.saturating_add(qty.saturating_sub(1));
            details.push(format!("{} {}-{}", reg_type, addr, end));
        } else {
            // Fallback: include raw trimmed tokens as detail items
            let parts: Vec<String> = entry
                .raw
                .split(|c: char| c.is_whitespace() || c == ':' || c == ',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            // Keep a few tokens as fallback
            if parts.len() > 0 {
                let take = std::cmp::min(4, parts.len());
                let join = parts[..take].join(" ");
                details.push(join);
            }
        }

        // Compose detail line as comma-separated items, items separated by single spaces inside
        let line2 = details.join(", ");

        // Line3: data bytes displayed as hex, comma separated, wrapped in brackets
        let mut data_items: Vec<String> = Vec::new();
        if !bytes.is_empty() {
            for b in &bytes {
                data_items.push(format!("{:02X}", b));
            }
        }
        let line3 = format!("[{}]", data_items.join(", "));

        (line1, line2, line3)
    }

    for i in 0..items_visible {
        let idx = view_offset.saturating_add(i);
        if let Some(entry) = logs.get(idx) {
            let (l1, l2, l3) = format_entry(entry);

            // Determine if this item is selected (we treat the topmost visible item as selected)
            let selected = i == 0;

            // Prefix: two-space area; show '>' in green when selected, otherwise two spaces
            let prefix_span = if selected {
                Span::styled("> ", Style::default().fg(Color::Green))
            } else {
                Span::raw("  ")
            };

            // Build three Lines with prefix (no background change; only marker differs)
            let line_one = Line::from(vec![prefix_span.clone(), Span::raw(l1)]);
            let line_two = Line::from(vec![Span::raw("  "), Span::raw(l2)]);
            let line_three = Line::from(vec![Span::raw("  "), Span::raw(l3)]);

            rendered_lines.push(line_one);
            rendered_lines.push(line_two);
            rendered_lines.push(line_three);
        } else {
            // blank item filler to keep layout
            rendered_lines.push(Line::from(Span::raw("")));
            rendered_lines.push(Line::from(Span::raw("")));
            rendered_lines.push(Line::from(Span::raw("")));
        }
    }

    let lines = rendered_lines;

    let title = if auto_scroll {
        Some(format!(
            " {} [Auto] ",
            lang().protocol.common.log_monitoring.clone()
        ))
    } else {
        Some(format!(
            " {} ",
            lang().protocol.common.log_monitoring.clone()
        ))
    };

    // Use the project's boxed paragraph helper which includes padding and optional scrollbar
    render_boxed_paragraph(
        frame,
        area,
        lines,
        view_offset,
        title.as_deref(),
        false,
        true,
    );

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
