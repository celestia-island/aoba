use ratatui::{prelude::*, widgets::*};

pub fn render_title(f: &mut Frame, area: Rect) {
    let title_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray));
    let title = Paragraph::new(crate::i18n::lang().title.as_str())
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .block(title_block);
    f.render_widget(title, area);
}
