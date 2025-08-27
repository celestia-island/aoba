use ratatui::{prelude::*, widgets::*};

use crate::tui::app::{App, Focus};

pub fn render_panels(f: &mut Frame, area: Rect, app: &App) {
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(60),
        ])
        .split(area);

    let items: Vec<ListItem> = app
        .ports
        .iter()
        .map(|p| {
            let label = format!("{} - {:?}", p.port_name, p.port_type);
            ListItem::new(label)
        })
        .collect();

    let mut left_block = Block::default()
        .title(format!(" {}", &crate::i18n::lang().com_ports))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain);
    if matches!(app.focus, Focus::Left) {
        left_block = left_block.style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    }

    let list = List::new(items).block(left_block).highlight_style(
        Style::default()
            .bg(Color::LightGreen)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if !app.ports.is_empty() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(list, chunks[0], &mut state);

    let mut right_block = Block::default()
        .title(format!(" {}", &crate::i18n::lang().details))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain);
    if matches!(app.focus, Focus::Right) {
        right_block = right_block.style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    }

    let details = if app.ports.is_empty() {
        Paragraph::new(crate::i18n::lang().no_com_ports.as_str()).block(right_block)
    } else {
        let p = &app.ports[app.selected];
        let text = format!(
            "{} {}\n{} {:?}\n\n{}",
            &crate::i18n::lang().name_label,
            p.port_name,
            &crate::i18n::lang().type_label,
            p.port_type,
            &crate::i18n::lang().details_placeholder
        );
        Paragraph::new(text).block(right_block)
    };

    f.render_widget(details, chunks[1]);
}
