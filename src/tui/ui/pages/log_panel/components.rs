use std::cmp::{max, min};

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::Line,
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::{i18n::lang, protocol::status::types};

/// Extract log data from current page state
pub fn extract_log_data() -> Option<(Vec<types::port::PortLogEntry>, usize, usize, bool)> {
    crate::protocol::status::read_status(|s| match &s.page {
        types::Page::ModbusLog { selected_port, .. } => {
            if let Some(port_name) = s.ports.order.get(*selected_port) {
                let pd = s.ports.map.get(port_name).cloned().unwrap_or_default();
                Ok(Some((
                    pd.logs.clone(),
                    pd.log_selected,
                    pd.log_view_offset,
                    pd.log_auto_scroll,
                )))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    })
    .ok()?
}

/// Render the main log display area
pub fn render_log_display(
    f: &mut Frame,
    area: Rect,
    logs: &[types::port::PortLogEntry],
    port_log_selected: usize,
    port_log_view_offset: usize,
    port_log_auto_scroll: bool,
) {
    let total_groups = logs.len();
    // We'll render a windowed view of log groups. Each group is 3 lines.
    let group_height = 3usize;

    // Inner height inside the block (account for borders)
    let inner_h = area.height.saturating_sub(2) as usize;
    let groups_per_screen = max(1usize, inner_h / group_height);

    // Determine bottom index based on auto-scroll or explicit offset (use per-port settings)
    let bottom = if total_groups == 0 {
        0usize
    } else if port_log_auto_scroll {
        total_groups.saturating_sub(1)
    } else {
        min(port_log_view_offset, total_groups.saturating_sub(1))
    };

    // Compute top group so that bottom aligns at the bottom of the visible area
    let top_group = (bottom + 1).saturating_sub(groups_per_screen);

    // Calculate available width for truncation (account for borders and scrollbar)
    let width = if area.width > 4 {
        (area.width - 4) as usize
    } else {
        10
    };

    let mut styled_lines: Vec<Line> = Vec::new();
    for (idx, g) in (top_group..min(total_groups, top_group + groups_per_screen)).enumerate() {
        if let Some(entry) = logs.get(g) {
            let selected = port_log_selected
                .checked_sub(top_group)
                .map(|s| s == idx)
                .unwrap_or(false);

            let prefix_text = if selected { "> " } else { "  " };
            // Direction: try to infer send/recv from parsed summary (best-effort)
            let is_send = entry
                .parsed
                .as_ref()
                .map(|p| {
                    let up = p.to_uppercase();
                    up.contains(" W ") || up.starts_with('W')
                })
                .unwrap_or(false);
            let dir_text = if is_send {
                lang().tabs.log_dir_send.as_str()
            } else {
                lang().tabs.log_dir_recv.as_str()
            };

            // Timestamp line: prefix + timestamp (with milliseconds) + 4 spaces + direction (direction styled bold + color)
            let ts = entry.when.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
            let mut ts_spans: Vec<Span> = Vec::new();
            ts_spans.push(Span::raw(prefix_text));
            ts_spans.push(Span::raw(ts));
            ts_spans.push(Span::raw("    "));
            // Direction style: bold + colored (green = Send / yellow = Receive). No background applied.
            let dir_span_style = if is_send {
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Green)
            } else {
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow)
            };
            ts_spans.push(Span::styled(dir_text, dir_span_style));
            styled_lines.push(Line::from(ts_spans));

            // Raw payload line: prefix + truncated raw
            let raw = entry.raw.replace('\n', " ");
            let raw_display = if raw.len() > width {
                let mut s = raw[..width].to_string();
                if s.len() >= 3 {
                    s.truncate(width.saturating_sub(3));
                    s.push_str("...");
                }
                s
            } else {
                raw
            };
            let raw_spans: Vec<Span> = vec![Span::raw(prefix_text), Span::raw(raw_display)];
            styled_lines.push(Line::from(raw_spans));

            // Parsed summary line
            let parsed_str = entry
                .parsed
                .clone()
                .unwrap_or_else(|| "(unparsed)".to_string());
            let parsed_spans: Vec<Span> = vec![Span::raw(prefix_text), Span::raw(parsed_str)];
            styled_lines.push(Line::from(parsed_spans));
        }
    }

    // Prepare a block with a small progress indicator in the title: " {selected}/{total}"
    let sel_display = if total_groups == 0 {
        0
    } else {
        port_log_selected + 1
    };
    // Compose follow label localized next to progress (e.g. "Follow latest" / "Free view").
    let follow_label = if port_log_auto_scroll {
        lang().tabs.log.hint_follow_on.as_str()
    } else {
        lang().tabs.log.hint_follow_off.as_str()
    };
    // Single-span title fallback: bold and color entire title depending on follow state.
    let title_text = format!(
        " {}{}/{}    {}",
        " ", sel_display, total_groups, follow_label
    );
    let title_span = Span::styled(
        title_text,
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(if port_log_auto_scroll {
                Color::Green
            } else {
                Color::Blue
            }),
    );

    let log_block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title(title_span);

    let log_para = Paragraph::new(styled_lines)
        .block(log_block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(log_para, area);
}

/// Render the input area at bottom of log panel
pub fn render_log_input(f: &mut Frame, area: Rect) {
    // Bottom text input for sending - simplified placeholder
    let input_block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title("Input (simplified)");
    let input_para = Paragraph::new("Log input placeholder").block(input_block);
    f.render_widget(input_para, area);
}

/// Check if we're in a subpage editing mode
pub fn is_in_subpage_editing() -> bool {
    false // Simplified for now
}

/// Check if a subpage is currently active
pub fn is_subpage_active() -> bool {
    if let Ok(v) = crate::protocol::status::read_status(|app| {
        Ok(matches!(
            app.page,
            types::Page::ModbusConfig { .. }
                | types::Page::ModbusDashboard { .. }
                | types::Page::ModbusLog { .. }
                | types::Page::About { .. }
        ))
    }) {
        v
    } else {
        false
    }
}
