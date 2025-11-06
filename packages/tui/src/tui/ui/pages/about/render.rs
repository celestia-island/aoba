use anyhow::Result;

use ratatui::prelude::*;

use crate::{i18n::lang, tui::{ status::read_status, ui::{ components::boxed_paragraph::render_boxed_paragraph, pages::about::components::{init_about_cache, render_about_page_manifest_lines}, }};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    Ok(vec![vec![lang()
        .hotkeys
        .hint_back_list
        .as_str()
        .to_string()]])
}

pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let content = init_about_cache();
    if let Ok(content) = content.lock() {
        let content = match render_about_page_manifest_lines(content.clone()) {
            Ok(c) => c,
            Err(_) => vec![Line::from("About (failed to render)")],
        };
        let offset = read_status(|status| {
            if let crate::tui::status::Page::About { view_offset } = &status.page {
                Ok(*view_offset)
            } else {
                Ok(0)
            }
        })?;

        render_boxed_paragraph(frame, area, content, offset, None, false, true);
    }

    Ok(())
}
