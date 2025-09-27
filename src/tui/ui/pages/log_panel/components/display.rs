use anyhow::Result;

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read},
};

/// Extract log data from current page state
pub fn extract_log_data() -> Result<Option<(Vec<types::port::PortLogEntry>, Option<usize>)>> {
    let res = read_status(|status| match &status.page {
        types::Page::LogPanel {
            selected_port,
            selected_item,
            ..
        } => {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Some(tuple) =
                        with_port_read(port, |pd| Some((pd.logs.clone(), *selected_item)))
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
    selected_item: Option<usize>,
) -> Result<()> {
    // Each log entry is rendered as a 3-line block
    let lines_per_item = 3usize;
    let content_height = area.height.saturating_sub(2) as usize; // Reserve space for borders
    let items_visible = std::cmp::max(1, content_height / lines_per_item);

    // Calculate which items to show based on selected_item
    let start_index = if let Some(selected_idx) = selected_item {
        // Manual mode: show selected item as first item
        selected_idx
    } else {
        // Auto-follow mode: show last items
        if logs.len() <= items_visible {
            0
        } else {
            logs.len() - items_visible
        }
    };

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
        let idx = start_index.saturating_add(i);
        if let Some(entry) = logs.get(idx) {
            let (l1, l2, l3) = format_entry(entry);

            // Determine if this item is selected
            let selected = if let Some(sel_idx) = selected_item {
                sel_idx == idx
            } else {
                // Auto-follow mode: select the last item
                idx == logs.len().saturating_sub(1)
            };

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

    // Build the title with internationalized text
    let log_title = Span::styled(
        lang().tabs.tab_log.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    );

    // Fix: Use selected_item to determine follow status, not auto_scroll
    let follow_status = if selected_item.is_none() {
        Span::styled(
            format!(" ({})", lang().tabs.log.hint_follow_on.clone()),
            Style::default().fg(Color::Green),
        )
    } else {
        Span::styled(
            format!(" ({})", lang().tabs.log.hint_follow_off.clone()),
            Style::default().fg(Color::Yellow),
        )
    };

    let title_line = Line::from(vec![
        Span::raw(" "),
        log_title,
        follow_status,
        Span::raw(" "),
    ]);

    // Create block with custom border and title
    let block = Block::default().borders(Borders::ALL).title(title_line);

    // Create inner area with 1 character left padding
    let inner = block.inner(area);
    let padded_area = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(1),
        height: inner.height,
    };

    // Render the block first
    frame.render_widget(block, area);

    // Then render the content with padding
    let paragraph = Paragraph::new(rendered_lines);
    frame.render_widget(paragraph, padded_area);

    // Add position counter at the bottom of the frame
    let current_pos = if let Some(sel_idx) = selected_item {
        sel_idx + 1
    } else {
        logs.len()
    };
    let total_items = logs.len();
    let position_text = format!("{} / {}", current_pos, total_items);

    // Render position counter at bottom-right of the frame
    let position_area = Rect {
        x: area.x + area.width.saturating_sub(position_text.len() as u16 + 2),
        y: area.y + area.height.saturating_sub(1),
        width: position_text.len() as u16 + 1,
        height: 1,
    };

    let position_paragraph =
        Paragraph::new(position_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(position_paragraph, position_area);

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
