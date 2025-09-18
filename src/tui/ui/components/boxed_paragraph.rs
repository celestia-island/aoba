use ratatui::{
    layout::{Constraint, Direction, Layout},
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
/// - `title`: Optional title - when provided, creates a 4:6 layout with title on left (40%) and content on right (60%)
/// - `custom_block`: Optional custom block - when provided, uses it instead of creating a default bordered block
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
    match title {
        Some(title_text) => {
            // Split into 4:6 layout (40% : 60%)
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
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
            render_content_area(frame, right_area, content, offset, wrap, show_scrollbar);
        }
        None => {
            // Original behavior - render content in full area
            render_content_area(frame, area, content, offset, wrap, show_scrollbar);
        }
    }
}

/// Helper function to render the content area with optional scrollbar
fn render_content_area(
    frame: &mut Frame,
    area: Rect,
    content: Vec<Line>,
    offset: usize,
    wrap: bool,
    show_scrollbar: bool,
) {
    let content_len = content.len();
    let content_len = content_len.saturating_sub((area.height / 2) as usize);
    let offset = std::cmp::min(offset, content_len.saturating_sub(1));

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::left(1));

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
