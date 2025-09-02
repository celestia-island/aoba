use crate::{i18n::lang, protocol::status::Status};

use ratatui::{prelude::*, widgets::*};

pub fn render_title(f: &mut Frame, area: Rect, app: &mut Status) {
    let title_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray));

    // If a subpage is active and a valid port is selected, show "{port_name} - {AOBA title}".
    let title_text = if let Some(_) = app.active_subpage {
        if !app.ports.is_empty() && app.selected < app.ports.len() {
            let p = &app.ports[app.selected];
            format!("{} - {}", p.port_name, lang().index.title.as_str())
        } else {
            lang().index.title.as_str().to_string()
        }
    } else {
        lang().index.title.as_str().to_string()
    };

    let title = Paragraph::new(title_text)
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .block(title_block);
    f.render_widget(title, area);
}
