use anyhow::Result;

use ratatui::{
    prelude::*,
    text::Span,
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, write_status},
    tui::ui::pages::about::components::{init_about_cache, render_about_page_manifest_lines},
};

// Return bottom hints for about page (same as entry, but no extras)
pub fn page_bottom_hints() -> Vec<Vec<String>> {
    vec![vec![lang().hotkeys.hint_back_list.as_str().to_string()]]
}

/// Render the about page. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title(Span::raw(format!(" {}", lang().index.title)));

    let content = init_about_cache();
    if let Ok(content) = content.lock() {
        let content = render_about_page_manifest_lines(content.clone());
        let content_len = content.len();
        let content_len = content_len - (area.height / 2) as usize;

        let offset = read_status(|s| {
            if let types::Page::About { view_offset } = &s.page {
                Ok(*view_offset)
            } else {
                Ok(0)
            }
        })?;
        let offset = std::cmp::min(offset, content_len.saturating_sub(1));
        write_status(|s| {
            if let types::Page::About { view_offset } = &mut s.page {
                *view_offset = offset;
            }
            Ok(())
        })?;

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

    Ok(())
}
