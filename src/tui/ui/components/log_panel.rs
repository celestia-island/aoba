use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::Line,
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::protocol::status::Status;

/// Render a log panel. Each log entry is presented as a 3-line grouped item:
/// 1) timestamp
/// 2) raw payload (single line, truncated)
/// 3) parsed summary (origin, R/W, command, slave id, range)
/// Navigation selects a whole group. Selected group is prefixed with "> " on the left
/// and rendered with a highlighted background; unselected groups get 2 spaces prefix.
pub fn render_log_panel(f: &mut Frame, area: Rect, app: &mut Status) {
    // Split area into top (logs) and bottom (input)
    let chunks: [Rect; 2] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Min(3),
        ratatui::layout::Constraint::Length(3),
    ])
    .areas(area);

    // Top: logs area
    let logs_area = chunks[0];
    // We'll render a windowed view of log groups. Each group is 3 lines.
    let total_groups = app.logs.len();
    let group_height = 3usize;

    // inner height inside the block (account for borders)
    let inner_h = logs_area.height.saturating_sub(2) as usize;
    let groups_per_screen = std::cmp::max(1usize, inner_h / group_height);

    // Determine bottom index based on auto-scroll or explicit offset
    let bottom = if total_groups == 0 {
        0usize
    } else if app.log_auto_scroll {
        total_groups.saturating_sub(1)
    } else {
        std::cmp::min(app.log_view_offset, total_groups.saturating_sub(1))
    };

    // Compute top group so that bottom aligns at the bottom of the visible area
    let top_group = if bottom + 1 >= groups_per_screen {
        bottom + 1 - groups_per_screen
    } else {
        0usize
    };

    // Calculate available width for truncation (account for borders and scrollbar)
    let width = if logs_area.width > 4 {
        (logs_area.width - 4) as usize
    } else {
        10
    };

    let mut styled_lines: Vec<Line> = Vec::new();
    for (idx, g) in
        (top_group..std::cmp::min(total_groups, top_group + groups_per_screen)).enumerate()
    {
        if let Some(entry) = app.logs.get(g) {
            let selected = app
                .log_selected
                .checked_sub(top_group)
                .map(|s| s == idx)
                .unwrap_or(false);

            let prefix_text = if selected { "> " } else { "  " };
            // direction: determine send/recv
            let is_send = entry
                .parsed
                .as_ref()
                .map(|p| p.rw.to_uppercase() == "W")
                .unwrap_or(false);
            let dir_text = if is_send { "发送" } else { "接收" };

            // timestamp line: prefix + timestamp (with milliseconds) + 4 spaces + direction (direction styled bold + color)
            let ts = entry.when.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
            let mut ts_spans: Vec<Span> = Vec::new();
            ts_spans.push(Span::raw(prefix_text));
            ts_spans.push(Span::raw(ts));
            ts_spans.push(Span::raw("    "));
            // direction style: bold + colored (green=发送 / yellow=接收). No background applied.
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
            // If selected, also apply background to the non-direction spans so selection has visible bg
            // build Line from spans vector
            let mut ts_line_spans: Vec<Span> = Vec::new();
            for s in ts_spans.into_iter() {
                ts_line_spans.push(s);
            }
            styled_lines.push(Line::from(ts_line_spans));

            // raw payload line: prefix + truncated raw
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

            // parsed summary line
            let parsed_str = if let Some(p) = &entry.parsed {
                format!(
                    "{} {} {} id={} @{} len= {}",
                    p.origin, p.rw, p.command, p.slave_id, p.address, p.length
                )
            } else {
                "(unparsed)".to_string()
            };
            let parsed_spans: Vec<Span> = vec![Span::raw(prefix_text), Span::raw(parsed_str)];
            styled_lines.push(Line::from(parsed_spans));
        }
    }

    // Prepare a block with a small progress indicator in the title: " {selected}/{total}"
    let sel_display = if total_groups == 0 {
        0
    } else {
        app.log_selected + 1
    };
    // compose follow label localized next to progress (e.g. " 跟随最新日志" / " 自由查看")
    let follow_label = if app.log_auto_scroll {
        crate::i18n::lang().hint_follow_on.as_str()
    } else {
        crate::i18n::lang().hint_follow_off.as_str()
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
            .fg(if app.log_auto_scroll {
                Color::Green
            } else {
                Color::Yellow
            }),
    );
    let block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title(title_span);

    // render content area (leave 1 column for scrollbar)
    let content_rect = Rect::new(
        logs_area.x,
        logs_area.y,
        logs_area.width.saturating_sub(1),
        logs_area.height,
    );
    let para = Paragraph::new(styled_lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(para, content_rect);

    // Draw a simple scrollbar at the right edge of logs_area
    if total_groups > groups_per_screen {
        let bar_x = logs_area.x + logs_area.width.saturating_sub(1);
        let bar_y = logs_area.y + 1; // inside top border
        let bar_h = logs_area.height.saturating_sub(2); // inside borders
        let denom = (total_groups.saturating_sub(groups_per_screen)) as f32;
        let ratio = if denom > 0.0 {
            (top_group as f32) / denom
        } else {
            0.0
        };
        let thumb_pos = bar_y + ((ratio * (bar_h.saturating_sub(1) as f32)).round() as u16);
        for i in 0..bar_h {
            let ch = if bar_y + i == thumb_pos { '█' } else { '│' };
            let p = Paragraph::new(ch.to_string()).style(Style::default().fg(Color::DarkGray));
            let r = Rect::new(bar_x, bar_y + i, 1, 1);
            f.render_widget(p, r);
        }
    }

    // Bottom: input area (fixed height)
    let input_area = chunks[1];
    crate::tui::ui::components::log_input::render_log_input(f, input_area, app);
}
