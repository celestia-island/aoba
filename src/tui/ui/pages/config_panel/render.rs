use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
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

    // Use render_boxed_paragraph as requested, no separate borders
    render_boxed_paragraph(frame, area, content, 0, None, false, true);

    Ok(())
}
