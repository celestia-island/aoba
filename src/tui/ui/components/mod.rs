pub mod error_msg;
pub mod log_input;
pub mod styled_label;

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};
use unicode_width::UnicodeWidthStr;

/// Produce a title span (bold) for a label. When `selected` is true the title is green; when
/// `editing` is true the title is yellow. Always bold to match existing UI conventions.
pub fn styled_title_span(label: &str, selected: bool, editing: bool) -> Span<'static> {
    let title_style = if selected && !editing {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if selected && editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    Span::styled(label.to_string(), title_style)
}

/// Render a boxed paragraph. Accepts a list of lines, a target rect, and an optional style for the
/// Paragraph content. The block will use all borders by default.
pub fn render_boxed_paragraph(frame: &mut Frame, area: Rect, content: Vec<Line>, offset: usize) {
    let content_len = content.len();
    let content_len = content_len - (area.height / 2) as usize;
    let offset = std::cmp::min(offset, content_len.saturating_sub(1));

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::left(1));
    let para = Paragraph::new(content)
        .block(block)
        .scroll((offset as u16, 0));
    frame.render_widget(para, area);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        &mut ScrollbarState::new(content_len).position(offset),
    );
}

/// Convert label/value pairs into aligned `Line`s. Each pair is (label, value, optional style)
/// `indent` is prefixed before each label (for example two spaces). `gap` is the number of
/// spaces between the label column and the value column.
pub fn kv_pairs_to_lines(pairs: &[(String, String)], gap: usize) -> Vec<Line<'static>> {
    let max_label_w = pairs
        .iter()
        .map(|(label, _)| UnicodeWidthStr::width(label.as_str()))
        .max()
        .unwrap_or(0usize);

    let mut out: Vec<Line> = Vec::new();

    for (label, value) in pairs.iter() {
        let lbl_w = UnicodeWidthStr::width(label.as_str());
        let fill = max_label_w.saturating_sub(lbl_w);
        let padded_label = format!("{}{}", label, " ".repeat(fill));

        out.push(Line::from(vec![
            Span::styled(padded_label, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" ".repeat(gap)),
            Span::raw(value.clone()),
        ]));
    }
    out
}
