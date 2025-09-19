use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::ui::{
        components::boxed_paragraph::render_boxed_paragraph,
        pages::config_panel::components::render_kv_lines_with_indicators,
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    vec![
        vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
        vec![lang().hotkeys.press_enter_modify.as_str().to_string()],
        vec![lang().hotkeys.hint_switch_options.as_str().to_string()],
    ]
}

pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    // Get the content lines with proper indicators
    let content = render_kv_lines_with_indicators()?;
    
    // Get the current view_offset from the page state
    let offset = read_status(|s| {
        if let types::Page::ModbusConfig { view_offset, .. } = &s.page {
            Ok(*view_offset)
        } else {
            Ok(0)
        }
    })?;

    // Use render_boxed_paragraph with scrolling offset
    render_boxed_paragraph(frame, area, content, offset, None, false, true);

    Ok(())
}
