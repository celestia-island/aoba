use std::cmp::{max, min};

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::Line,
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::{
    i18n::lang, protocol::status::Status,
    tui::utils::bus::Bus,
};

/// Render the log panel. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status) {
    let chunks: [Rect; 2] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Min(3),
        ratatui::layout::Constraint::Length(3),
    ])
    .areas(area);

    let logs_area = chunks[0];
    // We'll render a windowed view of log groups. Each group is 3 lines.
    let logs = app.page.logs.clone();
    let total_groups = logs.len();
    let group_height = 3usize;

    // Inner height inside the block (account for borders)
    let inner_h = logs_area.height.saturating_sub(2) as usize;
    let groups_per_screen = max(1usize, inner_h / group_height);

    // Determine bottom index based on auto-scroll or explicit offset
    let bottom = if total_groups == 0 {
        0usize
    } else if app.page.log_auto_scroll {
        total_groups.saturating_sub(1)
    } else {
        min(app.page.log_view_offset, total_groups.saturating_sub(1))
    };

    // Compute top group so that bottom aligns at the bottom of the visible area
    let top_group = (bottom + 1).saturating_sub(groups_per_screen);

    // Calculate available width for truncation (account for borders and scrollbar)
    let width = if logs_area.width > 4 {
        (logs_area.width - 4) as usize
    } else {
        10
    };

    let mut styled_lines: Vec<Line> = Vec::new();
    for (idx, g) in (top_group..min(total_groups, top_group + groups_per_screen)).enumerate() {
        if let Some(entry) = logs.get(g) {
            let selected = app
                .page
                .log_selected
                .checked_sub(top_group)
                .map(|s| s == idx)
                .unwrap_or(false);

            let prefix_text = if selected { "> " } else { "  " };
            // Direction: determine send / recv
            let is_send = entry
                .parsed
                .as_ref()
                .map(|p| p.rw.to_uppercase() == "W")
                .unwrap_or(false);
            let dir_text = if is_send {
                crate::i18n::lang().tabs.log_dir_send.as_str()
            } else {
                crate::i18n::lang().tabs.log_dir_recv.as_str()
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
        app.page.log_selected + 1
    };
    // Compose follow label localized next to progress (e.g. "Follow latest" / "Free view").
    let follow_label = if app.page.log_auto_scroll {
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
            .fg(if app.page.log_auto_scroll {
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
    f.render_widget(log_para, logs_area);

    // Bottom text input for sending - simplified placeholder
    let input_block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title("Input (simplified)");
    let input_para = Paragraph::new("Log input placeholder").block(input_block);
    f.render_widget(input_para, chunks[1]);
}

pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
    hints.push("f: Toggle follow".to_string());
    hints.push("c: Clear logs".to_string());

    // Append quit hint only when allowed (mirror global rule)
    let in_subpage_editing = app
        .page
        .subpage_form
        .as_ref()
        .map(|f| f.editing)
        .unwrap_or(false);
    let can_quit = !app.page.subpage_active && !in_subpage_editing;
    if can_quit {
        hints.push(lang().hotkeys.press_q_quit.as_str().to_string());
    }
    hints
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    // Log panel does not add extra mappings; let global mapping handle it
    None
}

/// Handle input for log panel. Sends commands via UiToCore.
pub fn handle_input(_key: crossterm::event::KeyEvent, bus: &Bus) -> bool {
    use crossterm::event::KeyCode as KC;

    match _key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Navigation commands
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Char('f') => {
            // Toggle follow mode
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Char('c') => {
            // Clear logs
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false
    }
}