use ratatui::{prelude::*, widgets::*};

use crate::tui::app::App;

pub fn render_bottom(f: &mut Frame, area: Rect, _app: &App) {
    let help_short = crate::i18n::lang().help_short.as_str();
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

        let instr = crate::i18n::lang().press_c_clear.as_str().to_string();
        let instr_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::DarkGray));
        let instr_p = Paragraph::new(instr)
            .alignment(Alignment::Center)
            .block(instr_block);
        f.render_widget(instr_p, rows[1]);
    } else {
        let help_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::DarkGray));
        let help = Paragraph::new(help_short)
            .alignment(Alignment::Center)
            .block(help_block);
        f.render_widget(help, area);
    }
}
