use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Borders,
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{Focus, Status},
};

pub fn render_ports(f: &mut Frame, area: Rect, app: &Status) {
    // Left: port table with name left-aligned and state right-aligned
    // Build lines manually so we can right-align the state column.
    let left_rect = area;
    let width = left_rect.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, p) in app.ports.iter().enumerate() {
        let name = p.port_name.clone();
        let state = app
            .port_states
            .get(i)
            .cloned()
            .unwrap_or(crate::protocol::status::PortState::Free);
        // Base style for state text
        let (state_text, state_style) = match state {
            crate::protocol::status::PortState::Free => {
                (lang().port_state_free.clone(), Style::default())
            }
            crate::protocol::status::PortState::OccupiedByThis => (
                lang().port_state_owned.clone(),
                Style::default().fg(Color::Green),
            ),
            crate::protocol::status::PortState::OccupiedByOther => (
                lang().port_state_other.clone(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        };

        // Compute padding to right-align state using display width (handles CJK)
        // Account for Block borders: the content area is two chars narrower
        let inner_width = width.saturating_sub(2);
        let name_w = UnicodeWidthStr::width(name.as_str());
        let state_w = UnicodeWidthStr::width(state_text.as_str());
        // Leave at least one space between name and state
        let pad = if inner_width > name_w + state_w {
            inner_width - name_w - state_w
        } else {
            1
        };
        let spacer = " ".repeat(pad);

        // Style port name based on occupancy
        let name_span = match state {
            crate::protocol::status::PortState::OccupiedByThis => Span::styled(
                name,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            crate::protocol::status::PortState::OccupiedByOther => Span::styled(
                name,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
            _ => Span::raw(name),
        };
        let mut spans = vec![name_span, Span::raw(spacer)];
        spans.push(Span::styled(state_text, state_style));

        // If selected, apply background highlight across the whole line by wrapping styled spans
        if i == app.selected {
            // Apply a background style to each span by re-styling
            let mut styled_spans: Vec<Span> = Vec::new();
            for sp in spans.into_iter() {
                let s = sp.style;
                let combined = s.bg(Color::LightGreen).add_modifier(Modifier::BOLD);
                styled_spans.push(Span::styled(sp.content, combined));
            }
            lines.push(Line::from(styled_spans));
        } else {
            lines.push(Line::from(spans));
        }
    }

    // Append two virtual items at the end: Refresh and Manual specify device
    // User-visible labels come from i18n
    let extra_labels = vec![
        lang().refresh_label.clone(),
        lang().manual_specify_label.clone(),
    ];
    // Compute display index base
    let base_idx = app.ports.len();
    for (j, lbl) in extra_labels.into_iter().enumerate() {
        let idx = base_idx + j;
        // Create a span for the label; use dim style for these virtual entries
        let span = Span::styled(lbl, Style::default().fg(Color::DarkGray));
        if idx == app.selected {
            let mut styled_spans: Vec<Span> = Vec::new();
            let combined = Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD);
            styled_spans.push(Span::styled(" ", combined));
            styled_spans.push(Span::styled(span.content, combined));
            lines.push(Line::from(styled_spans));
        } else {
            lines.push(Line::from(vec![span]));
        }
    }

    // If no ports, show a placeholder
    let left_content = if lines.is_empty() {
        vec![Line::from(Span::raw(lang().no_com_ports.clone()))]
    } else {
        lines
    };

    let mut left_block = Block::default().borders(Borders::ALL);
    // Only style the title text (not the leading space) when left panel is focused
    let title_text = lang().com_ports.as_str();
    let left_title = if matches!(app.focus, Focus::Left) {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title_text,
                Style::default().bg(Color::Gray).fg(Color::White),
            ),
        ])
    } else {
        Line::from(vec![Span::raw(" "), Span::raw(title_text)])
    };
    left_block = left_block.title(left_title);

    let mut paragraph = Paragraph::new(left_content).block(left_block);
    // Color convention: SELECTED panel should be darker; UNSELECTED lighter/white
    if matches!(app.focus, Focus::Right) {
        // Left is NOT selected
        paragraph = paragraph.style(Style::default().fg(Color::White));
    } else {
        // Left is selected
        paragraph = paragraph.style(Style::default().fg(Color::DarkGray));
    }
    f.render_widget(paragraph, area);
}
