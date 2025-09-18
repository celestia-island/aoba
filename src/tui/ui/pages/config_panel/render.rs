use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    tui::ui::{
        components::render_boxed_paragraph, pages::config_panel::components::render_kv_lines,
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    vec![
        vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
        vec![lang().hotkeys.press_enter_modify.as_str().to_string()],
    ]
}

pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let content = render_kv_lines()?;
    render_boxed_paragraph(frame, area, content, 0);

    Ok(())
}
