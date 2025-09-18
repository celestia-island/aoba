use anyhow::Result;

use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::ui::{
        components::render_boxed_paragraph,
        pages::about::components::{init_about_cache, render_about_page_manifest_lines},
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    vec![vec![lang().hotkeys.hint_back_list.as_str().to_string()]]
}

pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let content = init_about_cache();
    if let Ok(content) = content.lock() {
        let content = render_about_page_manifest_lines(content.clone());
        let offset = read_status(|s| {
            if let types::Page::About { view_offset } = &s.page {
                Ok(*view_offset)
            } else {
                Ok(0)
            }
        })?;

        render_boxed_paragraph(frame, area, content, offset, Some(lang().index.about_label.as_str()));
    }

    Ok(())
}
