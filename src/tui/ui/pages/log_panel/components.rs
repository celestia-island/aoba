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
        with_port_read, write_status,
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
    port_log_auto_scroll: bool,
) -> Result<()> {
    let total_groups = logs.len();
    let group_height = 3usize;

    let inner_h = area.height.saturating_sub(2) as usize;
    let groups_per_screen = max(1usize, inner_h / group_height);

    let bottom = if total_groups == 0 {
        0usize
    } else if port_log_auto_scroll {
        total_groups.saturating_sub(1)
    } else {
        read_status(|status| {
            if let types::Page::LogPanel { view_offset, .. } = &status.page {
                Ok(min(total_groups.saturating_sub(1), *view_offset))
            } else {
                Ok(0usize)
            }
        })
        .unwrap_or(0usize)
    };

    let top_group = (bottom + 1).saturating_sub(groups_per_screen);

    let width = if area.width > 4 {
        (area.width - 4) as usize
    } else {
        10
    };

    let mut styled_lines: Vec<Line> = Vec::new();
    for g in top_group..min(total_groups, top_group + groups_per_screen) {
        if let Some(entry) = logs.get(g) {
            let selected = read_status(|status| {
                if let types::Page::LogPanel { selected_port, .. } = &status.page {
                    if let Some(port_name) = status.ports.order.get(*selected_port) {
                        if let Some(port) = status.ports.map.get(port_name) {
                            if let Some((port_log_auto_scroll, len)) =
                                with_port_read(port, |pd| (pd.log_auto_scroll, pd.logs.len()))
                            {
                                let port_log_selected = if port_log_auto_scroll {
                                    len.saturating_sub(1)
                                } else if let types::Page::LogPanel { view_offset, .. } =
                                    &status.page
                                {
                                    min(len.saturating_sub(1), *view_offset)
                                } else {
                                    0usize
                                };
                                return Ok(g == port_log_selected);
                            }
                        }
                    }
                }
                Ok(false)
            })?;

            let prefix_text = if selected { "> " } else { "  " };
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

            let ts = entry.when.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
            let mut ts_spans: Vec<Span> = Vec::new();
            ts_spans.push(Span::raw(prefix_text));
            ts_spans.push(Span::raw(ts));
            ts_spans.push(Span::raw("    "));
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

            let parsed_str = entry
                .parsed
                .clone()
                .unwrap_or_else(|| "(unparsed)".to_string());
            let parsed_spans: Vec<Span> = vec![Span::raw(prefix_text), Span::raw(parsed_str)];
            styled_lines.push(Line::from(parsed_spans));
        }
    }

    let sel_display = if total_groups == 0 {
        0
    } else {
        1 // FIXME: should be selected + 1, but we don't track selected separately now
    };
    let follow_label = if port_log_auto_scroll {
        lang().tabs.log.hint_follow_on.as_str()
    } else {
        lang().tabs.log.hint_follow_off.as_str()
    };
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
    frame.render_widget(log_para, area);

    Ok(())
}

/// Render a small input area showing current mode and buffer. Height expected to be small (3 lines).
pub fn render_log_input(frame: &mut Frame, area: Rect) -> Result<()> {
    let mut lines: Vec<Line> = Vec::new();

    let input_mode = read_status(|status| {
        if let types::Page::LogPanel { input_mode, .. } = status.page {
            Ok(input_mode)
        } else {
            Ok(InputMode::Ascii) // Default to ASCII if not in LogPanel
        }
    })?;

    let content = if input_mode == InputMode::Hex {
        let mut s = String::new();
        let mut chars = read_status(|status| {
            Ok(status
                .temporarily
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
        read_status(|status| match &status.temporarily.input_raw_buffer {
            types::ui::InputRawBuffer::String {
                bytes: v,
                offset: _,
            } => Ok(String::from_utf8_lossy(v).into_owned()),
            types::ui::InputRawBuffer::Index(i) => Ok(i.to_string()),
            types::ui::InputRawBuffer::None => Ok(String::new()),
        })?
    };

    if content.is_empty() {
        let mode_text = match input_mode {
            InputMode::Ascii => lang().input.input_mode_ascii.as_str(),
            InputMode::Hex => lang().input.input_mode_hex.as_str(),
        };
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
        let spans = vec![
            Span::raw(" "), // Prefix editing line with single space (log list uses 2 incl. selector; here we keep compact)
            Span::styled(content.clone(), Style::default().fg(Color::Yellow)),
            Span::styled(" ", Style::default().bg(Color::Green).fg(Color::Black)),
        ];
        lines.push(ratatui::text::Line::from(spans));
    }

    lines.push(Line::from(Span::raw(" ")));

    let edit_hint = format!(
        "[Enter / i] {}",
        lang().input.hint_input_edit_short.as_str()
    );
    lines.push(Line::from(Span::raw(format!(" {edit_hint}"))));

    let (_title, block) = if !content.is_empty() {
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
    frame.render_widget(para, area);

    Ok(())
}

/// Scroll the LogPanel view offset up by `amount` (saturating at 0).
pub fn log_panel_scroll_up(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let types::Page::LogPanel { view_offset, .. } = &mut status.page {
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
    write_status(|status| {
        if let types::Page::LogPanel { view_offset, .. } = &mut status.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}
