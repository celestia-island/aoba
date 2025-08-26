use crate::tui::app::{App, Focus};
use ratatui::{prelude::*, widgets::*};

pub fn render_ui(f: &mut Frame, app: &App) {
    let area = f.area();
    let main_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(1)
        .constraints([
            ratatui::layout::Constraint::Length(1), // title
            ratatui::layout::Constraint::Min(0),
            ratatui::layout::Constraint::Length(3), // bottom help
        ])
        .split(area);

    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(1)
        .constraints([
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(60),
        ])
        .split(main_chunks[1]);

    // Title bar (centered, bold, deep green)
    let title = Paragraph::new(crate::i18n::tr("title"))
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Rgb(0, 150, 0))
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(title, main_chunks[0]);

    // Left: COM ports list
    let items: Vec<ListItem> = app
        .ports
        .iter()
        .map(|p| {
            let label = format!("{} - {:?}", p.port_name, p.port_type);
            ListItem::new(label)
        })
        .collect();

    let mut left_block = Block::default()
        .title(format!(" {}", crate::i18n::tr("com_ports")))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain);
    if matches!(app.focus, Focus::Left) {
        left_block = left_block.style(
            Style::default()
                .fg(Color::Rgb(0, 150, 0))
                .add_modifier(Modifier::BOLD),
        );
    }

    // Use deep-green highlight with white text for selected item
    let list = List::new(items).block(left_block).highlight_style(
        Style::default()
            .bg(Color::Rgb(0, 100, 0))
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if !app.ports.is_empty() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(list, chunks[0], &mut state);

    // Right: details (placeholder)
    let mut right_block = Block::default()
        .title(format!(" {}", crate::i18n::tr("details")))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain);
    if matches!(app.focus, Focus::Right) {
        right_block = right_block.style(
            Style::default()
                .fg(Color::Rgb(0, 150, 0))
                .add_modifier(Modifier::BOLD),
        );
    }

    let details = if app.ports.is_empty() {
        Paragraph::new(crate::i18n::tr("no_com_ports")).block(right_block)
    } else {
        let p = &app.ports[app.selected];
        let text = format!(
            "{} {}\n{} {:?}\n\n{}",
            crate::i18n::tr("name_label"),
            p.port_name,
            crate::i18n::tr("type_label"),
            p.port_type,
            crate::i18n::tr("details_placeholder")
        );
        Paragraph::new(text).block(right_block)
    };

    f.render_widget(details, chunks[1]);

    // Bottom: compact help (one line) + status (one line)
    let bottom = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(main_chunks[2]);

    let help_short = crate::i18n::tr("help_short");
    let help = Paragraph::new(help_short)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, bottom[0]);

    let status_prefix = if app.auto_refresh {
        crate::i18n::tr("auto_on")
    } else {
        crate::i18n::tr("auto_off")
    };
    let last = app
        .last_refresh
        .map(|t| format!("{} {}", crate::i18n::tr("last"), t.format("%H:%M:%S")))
        .unwrap_or_else(|| crate::i18n::tr("last_none"));
    let status_text = format!("{}    {}", status_prefix, last);
    let status = Paragraph::new(status_text)
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Rgb(0, 150, 0))
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(status, bottom[1]);
}
