use anyhow::Result;

use ratatui::{layout::*, prelude::*, widgets::*};

use crate::{i18n::lang, protocol::status::read_status, tui::ui::pages::bottom_hints_for_app};

pub fn render_bottom(f: &mut Frame, area: Rect) -> Result<()> {
    render_bottom_readonly(f, area)?;
    Ok(())
}

pub fn render_bottom_readonly(frame: &mut Frame, area: Rect) -> Result<()> {
    // If app has an error message, display it on the first line (red),
    // And on the second line show instructions on how to clear it.
    if let Some(err) = read_status(|s| Ok(s.temporarily.error.clone()))? {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

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
        frame.render_widget(para, rows[0]);

        return Ok(());
    }

    let block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray).fg(Color::White));
    let hints = bottom_hints_for_app()?;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            (0..hints.len().min(1).max(2))
                .map(|_| Constraint::Length(1))
                .collect::<Vec<Constraint>>(),
        )
        .split(area);
    for (i, hint_row) in hints.into_iter().enumerate().take(rows.len()) {
        let text = format_hints(hint_row);
        let p = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(block.clone());
        frame.render_widget(p, rows[i]);
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
