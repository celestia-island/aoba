use anyhow::Result;

use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::types,
    tui::{
        status::read_status,
        ui::{
            components::boxed_paragraph::render_boxed_paragraph,
            pages::config_panel::components::render_kv_lines_with_indicators,
        },
    },
};

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
            types::Page::ModbusDashboard { selected_port, .. }
            | types::Page::ConfigPanel { selected_port, .. }
            | types::Page::LogPanel { selected_port, .. } => *selected_port,
            types::Page::Entry {
                cursor: Some(types::cursor::EntryCursor::Com { index }),
                ..
            } => *index,
            _ => 0usize,
        })
    })?)?;

    let offset = read_status(|status| {
        if let types::Page::ConfigPanel { view_offset, .. } = &status.page {
            Ok(*view_offset)
        } else {
            Ok(0)
        }
    })?;

    render_boxed_paragraph(frame, area, content, offset, None, false, true);

    Ok(())
}
