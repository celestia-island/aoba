use std::cmp::min;

use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Paragraph},
};

use crate::i18n::lang;
use unicode_width::UnicodeWidthStr;

/// Return the hint fragments shown in the mode selector popup.
pub fn mode_selector_hints() -> Vec<String> {
    vec![
        lang().press_enter_select.as_str().to_string(),
        lang().press_esc_cancel.as_str().to_string(),
    ]
}

/// Render a centered mode selector popup. `index` is the currently selected option index.
pub fn render_mode_selector(f: &mut Frame, index: usize) {
    let area = f.area();
    let w = min(40, area.width.saturating_sub(4));
    let h = 6u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    // clear underlying widgets in this area and draw a solid dark background block
    f.render_widget(ratatui::widgets::Clear, popup);
    // Title forced to white so it remains visible regardless of theme
    let bg_block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .style(Style::default().bg(Color::DarkGray))
        .title(Span::styled(
            lang().mode_selector_title.as_str(),
            Style::default().fg(Color::White),
        ));
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

    // Overlay hint text on the bottom border line (left side) with a leading space.
    // We overwrite part of the horizontal border to place the hints without shifting layout.
    let hint_line_y = popup.y + popup.height - 1;
    if popup.height >= 1 && popup.width > 2 {
        let mut hint_text = format!(" {}", mode_selector_hints().join("  "));
        let max_w = popup.width.saturating_sub(2) as usize; // exclude corners
        if UnicodeWidthStr::width(hint_text.as_str()) > max_w {
            // Trim and add ellipsis if needed
            let mut acc = String::new();
            for ch in hint_text.chars() {
                let next = format!("{}{}", acc, ch);
                if UnicodeWidthStr::width(next.as_str()) > max_w.saturating_sub(1) {
                    // reserve 1 for ellipsis
                    acc.push('…');
                    break;
                }
                acc.push(ch);
            }
            hint_text = acc;
        }
        let hint_rect = Rect::new(popup.x + 1, hint_line_y, popup.width.saturating_sub(2), 1);
        let hint_para = Paragraph::new(hint_text)
            .alignment(ratatui::layout::Alignment::Left)
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));
        f.render_widget(hint_para, hint_rect);
    }
}
