use ratatui::{prelude::*, widgets::*};

use crate::{i18n::lang, protocol::status::Status, tui::ui::pages};

pub fn render_bottom(f: &mut Frame, area: Rect, _app: &mut Status) {
    let help_block = Block::default().borders(Borders::NONE);

    // If app has an error message, display it on the first line (red),
    // And on the second line show instructions on how to clear it.
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
        return;
    }

    // Normal (non-error) bottom rendering
    let help_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::White));

    // If a subpage is active, render two parallel hint lines: page-specific above and global below.
    if _app.active_subpage.is_some() {
        let rows = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .split(area);

        // Page-specific hints (above)
        let page_hints = pages::bottom_hints_for_app(_app);
        render_hints(f, rows[0], page_hints.iter().map(|s| s.as_str()));

        // Global hints (bottom-most)
        let global_hints = pages::global_hints_for_app(_app);
        render_hints(f, rows[1], global_hints.iter().map(|s| s.as_str()));
    } else {
        // Single-line bottom hints when not in a subpage
        let hints = pages::bottom_hints_for_app(_app);
        let text = format_hints(hints.iter().map(|s| s.as_str()));
        let help = Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(help_block);
        f.render_widget(help, area);
    }
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
