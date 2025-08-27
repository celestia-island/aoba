use ratatui::{prelude::*, widgets::*};

use crate::tui::app::App;

pub fn render_bottom(f: &mut Frame, area: Rect, _app: &App) {
    let help_short = crate::i18n::tr("help_short");
    let help_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray).fg(Color::DarkGray));
    let help = Paragraph::new(help_short)
        .alignment(Alignment::Center)
        .block(help_block);
    f.render_widget(help, area);
}
