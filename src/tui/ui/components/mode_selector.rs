use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Paragraph},
};

use crate::i18n::lang;

/// Render a centered mode selector popup. `index` is the currently selected option index.
pub fn render_mode_selector(f: &mut Frame, index: usize) {
    let area = f.area();
    let w = std::cmp::min(40, area.width.saturating_sub(4));
    let h = 6u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    // clear underlying widgets in this area and draw a solid dark background block
    f.render_widget(ratatui::widgets::Clear, popup);
    let bg_block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .style(Style::default().bg(Color::DarkGray))
        .title(" 选择模式");
    f.render_widget(bg_block, popup);

    // inner area for text (inside borders)
    let inner = Rect::new(
        popup.x + 1,
        popup.y + 1,
        popup.width.saturating_sub(2),
        popup.height.saturating_sub(2),
    );

    // Options (each line centered horizontally, and vertically centered in inner area)
    let options = vec![lang().tab_master.as_str(), lang().tab_slave.as_str()];
    let mut lines: Vec<ratatui::text::Line> = Vec::new();
    for (i, o) in options.iter().enumerate() {
        let span = if i == index {
            ratatui::text::Span::styled(*o, Style::default().fg(Color::White).bg(Color::LightBlue))
        } else {
            ratatui::text::Span::styled(*o, Style::default().fg(Color::White).bg(Color::DarkGray))
        };
        lines.push(ratatui::text::Line::from(span));
    }

    // vertical center: compute start y so the list is centered within inner.height
    let opts_h = lines.len() as u16;
    let start_y = if inner.height > opts_h {
        inner.y + (inner.height - opts_h) / 2
    } else {
        inner.y
    };
    let opts_rect = Rect::new(inner.x, start_y, inner.width, opts_h);
    let opts_para = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(opts_para, opts_rect);
}
