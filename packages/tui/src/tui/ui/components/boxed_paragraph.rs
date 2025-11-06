use ratatui::{
    prelude::*,
    text::Line,
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

/// Render a boxed paragraph with comprehensive options.
///
/// Parameters:
/// - `frame`: The frame to render to
/// - `area`: The area to render in
/// - `content`: The lines of content to display
/// - `offset`: Scroll offset for the content
/// - `title`: Optional title - when provided, applies it to the block title
/// - `wrap`: Whether to enable text wrapping
/// - `show_scrollbar`: Whether to show the scrollbar
pub fn render_boxed_paragraph(
    frame: &mut Frame,
    area: Rect,
    content: Vec<Line>,
    offset: usize,
    title: Option<&str>,
    wrap: bool,
    show_scrollbar: bool,
) {
    let content_len = content.len();
    let content_len = content_len.saturating_sub((area.height / 2) as usize);
    let offset = std::cmp::min(offset.saturating_sub(2), content_len.saturating_sub(1));

    // Create block with optional title
    let mut block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::left(1));

    if let Some(title_text) = title {
        block = block.title(format!(" {title_text} "));
    }

    let mut para = Paragraph::new(content)
        .block(block)
        .scroll((offset as u16, 0));

    if wrap {
        para = para.wrap(ratatui::widgets::Wrap { trim: true });
    }

    frame.render_widget(para, area);

    if show_scrollbar {
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut ScrollbarState::new(content_len).position(offset),
        );
    }
}
