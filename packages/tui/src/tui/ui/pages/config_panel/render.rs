use anyhow::Result;

use ratatui::prelude::*;

use crate::{i18n::lang, tui::status as types, tui::status::read_status, tui::ui::components::boxed_paragraph::render_boxed_paragraph, tui::ui::pages::config_panel::components::render_kv_lines_with_indicators};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    Ok(vec![
        vec![
            lang().hotkeys.hint_move_vertical.as_str().to_string(),
            lang().hotkeys.press_enter_modify.as_str().to_string(),
        ],
        vec![lang().hotkeys.hint_switch_options.as_str().to_string()],
    ])
}

pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let content = render_kv_lines_with_indicators(read_status(|status| {
        Ok(match &status.page {
            crate::tui::status::Page::ModbusDashboard { selected_port, .. }
            | crate::tui::status::Page::ConfigPanel { selected_port, .. }
            | crate::tui::status::Page::LogPanel { selected_port, .. } => *selected_port,
            crate::tui::status::Page::Entry {
                cursor: Some(types::cursor::EntryCursor::Com { index }),
                ..
            } => *index,
            _ => 0usize,
        })
    })?)?;

    let offset = read_status(|status| {
        if let crate::tui::status::Page::ConfigPanel { view_offset, .. } = &status.page {
            Ok(*view_offset)
        } else {
            Ok(0)
        }
    })?;

    render_boxed_paragraph(frame, area, content, offset, None, false, true);

    Ok(())
}
