use anyhow::Result;

use ratatui::{layout::*, prelude::*, widgets::*};

use crate::{i18n::lang, protocol::status::read_status, tui::ui::pages::bottom_hints_for_app};

pub fn render_bottom(frame: &mut Frame, area: Rect) -> Result<()> {
    // Cumulative rendering: we render page-provided bottom hints and also
    // append transient error lines (error message + clear hint) if present.
    let hints = bottom_hints_for_app().unwrap_or_else(|_| vec![]);
    let err_opt = read_status(|s| Ok(s.temporarily.error.clone()))?;

    let err_lines = if err_opt.is_some() { 2usize } else { 0usize };
    let rows_count = hints.len() + err_lines;

    if rows_count == 0 {
        // Nothing to render in bottom
        return Ok(());
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            (0..rows_count)
                .map(|_| Constraint::Length(1))
                .collect::<Vec<Constraint>>(),
        )
        .split(area);

    // If error present, render it on the topmost lines of the bottom area.
    let mut next_row = 0usize;
    if let Some(err) = err_opt {
        let err_block = Block::default().borders(Borders::NONE).clone().style(
            Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        let (msg, _ts) = (&err.message, &err.timestamp);
        let para = Paragraph::new(msg.as_str())
            .alignment(Alignment::Left)
            .block(err_block);
        frame.render_widget(para, rows[next_row]);
        next_row += 1;

        // clear hint line
        let clear_hint = lang().hotkeys.press_c_clear.as_str().to_string();
        let hint_block = Block::default()
            .borders(Borders::NONE)
            .style(Style::default().bg(Color::Gray).fg(Color::White));
        let hint_para = Paragraph::new(clear_hint)
            .alignment(Alignment::Center)
            .block(hint_block);
        frame.render_widget(hint_para, rows[next_row]);
        next_row += 1;
    }

    let block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray).fg(Color::White));
    // Render remaining hint rows (if any) after error lines
    for (i, hint_row) in hints
        .into_iter()
        .enumerate()
        .take(rows_count.saturating_sub(err_lines))
    {
        let text = format_hints(hint_row);
        let p = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(block.clone());
        frame.render_widget(p, rows[next_row + i]);
    }

    Ok(())
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
