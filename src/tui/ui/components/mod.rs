pub mod config_panel;
pub mod log_input;
pub mod log_panel;
pub mod master_list_panel;
pub mod mode_selector;
pub mod pull_list_panel;

use ratatui::{
    prelude::*,
    style::Style,
    widgets::{Block, Paragraph},
};

/// Render a boxed paragraph. Accepts a list of lines, a target rect, and an optional style for the
/// paragraph content. The block will use all borders by default.
pub fn render_boxed_paragraph(
    f: &mut Frame,
    area: Rect,
    lines: Vec<ratatui::text::Line>,
    style: Option<Style>,
) {
    let block = Block::default().borders(ratatui::widgets::Borders::ALL);
    let mut para = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    if let Some(s) = style {
        para = para.style(s);
    }
    f.render_widget(para, area);
}
