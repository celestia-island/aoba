pub mod boxed_paragraph;
pub mod error_msg;
pub mod input_span_handler;
pub mod styled_label;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
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
