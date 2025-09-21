use anyhow::Result;
use std::cmp::{max, min};

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    i18n::lang,
    protocol::status::{
        read_status,
        types::{self, ui::InputMode},
        write_status,
    },
};

/// Extract log data from current page state
pub fn extract_log_data() -> Option<(Vec<types::port::PortLogEntry>, usize, usize, bool)> {
    read_status(|s| match &s.page {
        types::Page::LogPanel { selected_port, .. } => {
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

/// Render a small input area showing current mode and buffer. Height expected to be small (3 lines).
pub fn render_log_input(f: &mut Frame, area: Rect) -> Result<()> {
    let mut lines: Vec<Line> = Vec::new();

    // For now, use ASCII mode as default since we don't have per-port input modes
    let input_mode = read_status(|s| {
        if let types::Page::LogPanel { input_mode, .. } = s.page {
            Ok(input_mode)
        } else {
            Ok(InputMode::Ascii) // Default to ASCII if not in LogPanel
        }
    })?;

    // Show buffer on the first content line (right under the title)
    let content = if input_mode == InputMode::Hex {
        let mut s = String::new();
        let mut chars = read_status(|s| {
            Ok(s.temporarily
                .input_raw_buffer
                .as_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>())
        })?;
        if chars.len() % 2 == 1 {
            chars.push('_');
        }
        for (i, ch) in chars.chars().enumerate() {
            if i > 0 && i % 2 == 0 {
                s.push(' ');
            }
            s.push(ch);
        }
        s
    } else {
        read_status(|s| match &s.temporarily.input_raw_buffer {
            types::ui::InputRawBuffer::String(v) => Ok(String::from_utf8_lossy(v).into_owned()),
            types::ui::InputRawBuffer::Index(i) => Ok(i.to_string()),
            types::ui::InputRawBuffer::None => Ok(String::new()),
        })?
    };

    // If buffer empty and not editing, show faint gray italic placeholder indicating current input mode
    if content.is_empty() {
        // Show as: " {input_mode_current} {mode_text}" with a leading space to align with title
        let mode_text = match input_mode {
            InputMode::Ascii => lang().input.input_mode_ascii.as_str(),
            InputMode::Hex => lang().input.input_mode_hex.as_str(),
        };
        // Add extra leading spaces to align with title
        let placeholder = format!(
            " {} {}",
            lang().input.input_mode_current.as_str(),
            mode_text
        ); // Keep single leading space
        lines.push(Line::from(Span::styled(
            placeholder,
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )));
    } else {
        // Style editing content in yellow (match config page editing color)
        // Build spans in-place
        let spans = vec![
            Span::raw(" "), // Prefix editing line with single space (log list uses 2 incl. selector; here we keep compact)
            Span::styled(content.clone(), Style::default().fg(Color::Yellow)),
            // Visual cursor block (green background)
            Span::styled(" ", Style::default().bg(Color::Green).fg(Color::Black)),
        ];
        lines.push(ratatui::text::Line::from(spans));
    }

    // Keep a spare empty middle line so layout remains 3 lines
    // Spare empty middle line should also keep two spaces for alignment
    lines.push(Line::from(Span::raw(" ")));

    // Hint (short)
    // Show dual-key hint: Enter / i
    let edit_hint = format!(
        "[Enter / i] {}",
        lang().input.hint_input_edit_short.as_str()
    );
    lines.push(Line::from(Span::raw(format!(" {edit_hint}"))));

    // Choose title and block based on whether we're editing
    let (_title, block) = if !content.is_empty() {
        // Highlight title when editing (use library green)
        let title = Span::styled(
            format!(" {} ", lang().input.input_label.as_str()),
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
        (
            title.clone(),
            Block::default().borders(Borders::ALL).title(title),
        )
    } else {
        let title_text = format!(" {} ", lang().input.input_label.as_str());
        (
            Span::raw(title_text.clone()),
            Block::default().borders(Borders::ALL).title(title_text),
        )
    };

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(para, area);

    Ok(())
}

/// Check if we're in a subpage editing mode
pub fn is_in_subpage_editing() -> bool {
    false // Simplified for now
}

/// Check if a subpage is currently active
pub fn is_subpage_active() -> bool {
    if let Ok(v) = read_status(|app| {
        Ok(matches!(
            app.page,
            types::Page::ConfigPanel { .. }
                | types::Page::ModbusDashboard { .. }
                | types::Page::LogPanel { .. }
                | types::Page::About { .. }
        ))
    }) {
        v
    } else {
        false
    }
}

/// Scroll the LogPanel view offset up by `amount` (saturating at 0).
pub fn log_panel_scroll_up(amount: usize) -> anyhow::Result<()> {
    write_status(|s| {
        if let types::Page::LogPanel { view_offset, .. } = &mut s.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the LogPanel view offset down by `amount`.
pub fn log_panel_scroll_down(amount: usize) -> anyhow::Result<()> {
    write_status(|s| {
        if let types::Page::LogPanel { view_offset, .. } = &mut s.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}
