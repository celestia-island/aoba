use ratatui::{prelude::*, widgets::*};

use crate::{protocol::status::Status, tui::ui::pages};

pub fn render_bottom(f: &mut Frame, area: Rect, _app: &Status) {
    let help_block = Block::default().borders(Borders::NONE);

    // If app has an error message, display it on the first line (red),
    // and on the second line show instructions on how to clear it.
    if let Some(err) = &_app.error {
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
        let msg = &err.0;
        let p = Paragraph::new(msg.as_str())
            .alignment(Alignment::Left)
            .block(err_block);
        f.render_widget(p, rows[0]);

        // Delegate construction of bottom hints to page layer so behavior is consistent.
        let hints = crate::tui::ui::pages::bottom_hints_for_app(_app);
        let hint_rect = rows[1];
        // Use the unified renderer for hints
        render_hints(f, hint_rect, hints.iter().map(|s| s.as_str()));
    } else {
        let help_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::White));

        // Delegate to page layer to assemble bottom hints.
        let hints = pages::bottom_hints_for_app(_app);
        let text = format_hints(hints.iter().map(|s| s.as_str()));
        let help = Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(help_block);
        f.render_widget(help, area);
    }
}

// Separator used between hint fragments. Public so other modules can reuse.
pub const HINT_SEPARATOR: &str = "   ";

/// Join hint fragments into a single string using the project's hint separator.
/// Accepts any iterator of items convertible to &str.
pub fn format_hints<I, S>(hints: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    hints
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect::<Vec<String>>()
        .join(HINT_SEPARATOR)
}

/// Render hints into the given `area` using the project's standard hint style and separator.
pub fn render_hints<'a, I>(f: &mut Frame, area: Rect, hints: I)
where
    I: IntoIterator<Item = &'a str>,
{
    let text = hints
        .into_iter()
        .collect::<Vec<&str>>()
        .join(HINT_SEPARATOR);
    let hint_block = Block::default().style(Style::default().bg(Color::Gray).fg(Color::White));
    let hint_para = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(hint_block);
    f.render_widget(hint_para, area);
}
