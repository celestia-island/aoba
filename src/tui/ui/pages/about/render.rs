use anyhow::Result;

use ratatui::{
    prelude::*,
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
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
        let para = Paragraph::new(render_about_page_manifest_lines(content.clone()))
            .block(block)
            .scroll(read_status(|s| {
                if let types::Page::About { view_offset } = &s.page {
                    Ok((*view_offset as u16, 0u16))
                } else {
                    Ok((0u16, 0u16))
                }
            })?);
        frame.render_widget(para, area);
    }

    Ok(())
}
