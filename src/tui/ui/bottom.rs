use ratatui::{prelude::*, widgets::*};

use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
};
use unicode_width::UnicodeWidthStr;

pub fn render_bottom(f: &mut Frame, area: Rect, app: &mut Status) {
    render_bottom_readonly(f, area, app);
}

pub fn render_bottom_readonly(f: &mut Frame, area: Rect, app: &Status) {
    let help_block = Block::default().borders(Borders::NONE);

    // If app has an error message, display it on the first line (red),
    // And on the second line show instructions on how to clear it.
    if let Some(err) = app.temporarily.error.as_ref() {
        // Split the provided area into two rows
        let rows = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .split(area);

        let err_block = help_block.clone().style(
            Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        let (msg, _ts) = (&err.message, &err.timestamp);
        let p = Paragraph::new(msg.as_str())
            .alignment(Alignment::Left)
            .block(err_block);
        f.render_widget(p, rows[0]);

        // Delegate construction of bottom hints to page layer so behavior is consistent.
        let hints = crate::tui::ui::pages::bottom_hints_for_app(app);
        let hint_rect = rows[1];
        // Use the unified renderer for hints
        render_hints(f, hint_rect, hints.iter().map(|s| s.as_str()));
        return;
    }

    // Normal (non-error) bottom rendering
    let help_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::White));

    // Determine if a per-port log-clear confirmation is pending (keeps legacy behavior).
    let mut port_log_clear_pending = false;
    if let types::Page::ModbusLog { selected_port } = app.page {
        if let Some(port_name) = app.ports.order.get(selected_port) {
            if let Some(pdata) = app.ports.map.get(port_name) {
                port_log_clear_pending = pdata.log_clear_pending;
            }
        }
    }

    // Obtain page-provided hints arrays and let them drive rendering decisions.
    // Support multi-line hints: split any hint string on '\n' to produce logical rows.
    let raw_page_hints = crate::tui::ui::pages::bottom_hints_for_app(app);
    let raw_global_hints = crate::tui::ui::pages::global_hints_for_app(app);

    // Expand hints into individual rows by splitting on newlines inside each fragment.
    let mut page_rows: Vec<String> = Vec::new();
    for s in raw_page_hints.iter() {
        for line in s.split('\n') {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                page_rows.push(trimmed.to_string());
            }
        }
    }
    let mut global_rows: Vec<String> = Vec::new();
    for s in raw_global_hints.iter() {
        for line in s.split('\n') {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                global_rows.push(trimmed.to_string());
            }
        }
    }

    if port_log_clear_pending {
        // Prefer to show three rows when possible: confirmation, page hints, global hints.
        let avail = area.height as usize;
        if avail >= 3 {
            let rows = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .margin(0)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(area);

            // Confirmation row (yellow background)
            let confirm_block = Block::default().style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );
            let confirm_text = lang().hotkeys.press_c_confirm.as_str();
            let confirm_para = Paragraph::new(confirm_text)
                .block(confirm_block)
                .alignment(Alignment::Center);
            f.render_widget(confirm_para, rows[0]);

            // Page-specific hints (middle)
            render_hints(f, rows[1], page_rows.iter().map(|s| s.as_str()));

            // Global hints (bottom-most)
            render_hints(f, rows[2], global_rows.iter().map(|s| s.as_str()));
            return;
        } else if avail == 2 {
            // Render confirmation + collapsed hints line
            let rows = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .margin(0)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(area);

            let confirm_block = Block::default().style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );
            let confirm_text = lang().hotkeys.press_c_confirm.as_str();
            let confirm_para = Paragraph::new(confirm_text)
                .block(confirm_block)
                .alignment(Alignment::Center);
            f.render_widget(confirm_para, rows[0]);

            // collapse remaining hints into one line
            let mut combined: Vec<String> = Vec::new();
            combined.extend(page_rows.clone());
            combined.extend(global_rows.clone());
            render_hints(f, rows[1], combined.iter().map(|s| s.as_str()));
            return;
        } else if avail == 1 {
            // Only show confirmation row (we have no room for hints)
            let block = Block::default().style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );
            let text = lang().hotkeys.press_c_confirm.as_str();
            let para = Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Center);
            f.render_widget(para, area);
            return;
        } else {
            return; // no space
        }
    }

    // Simplified decision tree using sequential checks:
    // 1) nothing to show, 2) only page hints, 3) only global hints, 4) both -> two rows.
    // Determine how many rows we can display
    let avail_rows = area.height as usize;
    if avail_rows == 0 {
        return;
    }

    // If we can render all rows individually, we usually do so. However, to
    // follow a "minimum-occupied-rows" policy we first try to pack page and
    // global hints into fewer rows when it makes sense (e.g. both fit in one
    // line width) to avoid leaving an extra empty-looking row.
    let total_needed = page_rows.len() + global_rows.len();
    if avail_rows >= total_needed && total_needed > 0 {
        // Prepare joined forms for simple packing checks
        let page_joined = format_hints(page_rows.iter().map(|s| s.as_str()));
        let global_joined = format_hints(global_rows.iter().map(|s| s.as_str()));

        // If everything can fit into a single line, render as one row to be
        // minimally occupying. We measure by character width compared to
        // available columns; this is a heuristic (doesn't account for full
        // grapheme widths) but is sufficient for the typical ascii hints.
        let combined = if !page_joined.is_empty() {
            if !global_joined.is_empty() {
                format!("{}{}{}", page_joined, "    ", global_joined)
            } else {
                page_joined.clone()
            }
        } else {
            global_joined.clone()
        };

        if !combined.is_empty() && (UnicodeWidthStr::width(combined.as_str()) as u16) <= area.width
        {
            // Single-line fit: render one centered hint row
            let help = Paragraph::new(combined)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(help_block);
            f.render_widget(help, area);
            return;
        }

        // Otherwise fall back to rendering each row individually (page rows
        // first, then global rows) since there's room for them.
        let mut constraints: Vec<ratatui::layout::Constraint> = Vec::new();
        for _ in 0..total_needed {
            constraints.push(ratatui::layout::Constraint::Length(1));
        }
        let rows = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints(constraints.as_slice())
            .split(area);

        let mut idx = 0usize;
        for pr in page_rows.iter() {
            render_hints(f, rows[idx], std::iter::once(pr.as_str()));
            idx += 1;
        }
        for gr in global_rows.iter() {
            render_hints(f, rows[idx], std::iter::once(gr.as_str()));
            idx += 1;
        }
        return;
    }

    // If we don't have enough rows, collapse to at most two rows:
    // - if avail_rows == 1: join all rows into a single line
    // - if avail_rows >= 2: render two joined rows: page-joined and global-joined
    if avail_rows == 1 {
        let mut all: Vec<String> = Vec::new();
        all.extend(page_rows.clone());
        all.extend(global_rows.clone());
        let text = format_hints(all.iter().map(|s| s.as_str()));
        let help = Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(help_block);
        f.render_widget(help, area);
        return;
    }

    // avail_rows >= 2 but less than total_needed: render two joined rows
    let rows = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(area);

    let page_joined = format_hints(page_rows.iter().map(|s| s.as_str()));
    let global_joined = format_hints(global_rows.iter().map(|s| s.as_str()));
    render_hints(f, rows[0], std::iter::once(page_joined.as_str()));
    render_hints(f, rows[1], std::iter::once(global_joined.as_str()));
}

/// Join hint fragments into a single string.
pub fn format_hints<I, S>(hints: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    // Use a wider visual gap between bottom hints: four spaces.
    let sep = "    ";
    hints
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect::<Vec<String>>()
        .join(sep)
}

/// Format a key / value shortcut hint, e.g. key = "i", value = "Edit" -> "i=Edit".
/// Provided here so pages / components can register consistent kv-styled hints.
pub fn format_kv_hint(key: &str, value: &str) -> String {
    // Use localized template, replace {key} and {label}
    let tmpl = lang().hotkeys.hint_kv_template.as_str();
    tmpl.replace("{key}", key).replace("{label}", value)
}

/// Render hints into the given `area` using the project's standard hint style and separator.
pub fn render_hints<'a, I>(f: &mut Frame, area: Rect, hints: I)
where
    I: IntoIterator<Item = &'a str>,
{
    // Use a wider visual gap between bottom hints: four spaces.
    let sep = "    ";
    let text = hints.into_iter().collect::<Vec<&str>>().join(sep);
    let hint_block = Block::default().style(Style::default().bg(Color::Gray).fg(Color::White));
    let hint_para = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(hint_block);
    f.render_widget(hint_para, area);
}
