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
    layout::{Constraint, Direction, Layout},
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
    render_boxed_paragraph_with_block(frame, area, content, offset, None);
}

/// Render a boxed paragraph with optional custom block. When block is provided, uses it instead
/// of creating a default bordered block. When title is provided and block is None, creates a 4:6 layout.
pub fn render_boxed_paragraph_with_title(
    frame: &mut Frame, 
    area: Rect, 
    content: Vec<Line>, 
    offset: usize,
    title: Option<&str>
) {
    match title {
        Some(title_text) => {
            // Split into 4:6 layout (40% : 60%)
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(60),
                ])
                .split(area);

            let left_area = chunks[0];
            let right_area = chunks[1];

            // Render title in left area
            let title_block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", title_text));
            let title_para = Paragraph::new(Vec::<Line>::new()).block(title_block);
            frame.render_widget(title_para, left_area);

            // Render content in right area
            render_content_area(frame, right_area, content, offset, None, false);
        }
        None => {
            // Original behavior - render content in full area
            render_content_area(frame, area, content, offset, None, false);
        }
    }
}

/// Render a boxed paragraph with optional custom block.
pub fn render_boxed_paragraph_with_block(
    frame: &mut Frame, 
    area: Rect, 
    content: Vec<Line>, 
    offset: usize,
    custom_block: Option<Block<'_>>
) {
    render_content_area(frame, area, content, offset, custom_block, false);
}

/// Render a boxed paragraph with optional custom block and wrapping support.
pub fn render_boxed_paragraph_with_block_and_wrap(
    frame: &mut Frame, 
    area: Rect, 
    content: Vec<Line>, 
    offset: usize,
    custom_block: Option<Block<'_>>,
    wrap: bool
) {
    render_content_area(frame, area, content, offset, custom_block, wrap);
}

/// Helper function to render the content area with scrollbar
fn render_content_area(frame: &mut Frame, area: Rect, content: Vec<Line>, offset: usize, custom_block: Option<Block<'_>>, wrap: bool) {
    let content_len = content.len();
    let content_len = content_len - (area.height / 2) as usize;
    let offset = std::cmp::min(offset, content_len.saturating_sub(1));

    let block = custom_block.unwrap_or_else(|| {
        Block::default()
            .borders(Borders::ALL)
            .padding(Padding::left(1))
    });
    
    let mut para = Paragraph::new(content)
        .block(block)
        .scroll((offset as u16, 0));
    
    if wrap {
        para = para.wrap(ratatui::widgets::Wrap { trim: true });
    }
    
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
