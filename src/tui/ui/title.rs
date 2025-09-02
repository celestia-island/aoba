use crate::{i18n::lang, protocol::status::Status};
use ratatui::{prelude::*, widgets::*};

pub fn render_title(f: &mut Frame, area: Rect, app: &mut Status) {
    // Horizontal layout: left (spinner) + center (title) + right (reserved)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    // Background bar
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(Color::Gray));
    f.render_widget(bg_block, area);

    // Spinner (top-left)
    if app.busy {
        let frames = ["●○○", "○●○", "○○●"];
        let ch = frames[(app.spinner_frame as usize) % frames.len()];
        let spin = Paragraph::new(ch).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(spin, chunks[0]);
    }

    // Title text (center area)
    let title_text = if app.active_subpage.is_some() {
        if !app.ports.is_empty() && app.selected < app.ports.len() {
            let p = &app.ports[app.selected];
            format!("{} - {}", p.port_name, lang().index.title.as_str())
        } else {
            lang().index.title.as_str().to_string()
        }
    } else {
        lang().index.title.as_str().to_string()
    };

    let title_para = Paragraph::new(title_text)
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(title_para, chunks[1]);
}
