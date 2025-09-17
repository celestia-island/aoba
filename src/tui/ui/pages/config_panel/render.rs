use anyhow::Result;
use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::read_status,
    tui::ui::pages::config_panel::components::{
        extract_config_snapshot, is_subpage_active, render_kv_panel, render_simplified_content,
    },
};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    vec![
        vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
        vec![lang().hotkeys.press_enter_modify.as_str().to_string()],
    ]
}

/// Render a configuration panel for a subpage. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect, style: Option<Style>) -> Result<()> {
    read_status(|app| {
        if !is_subpage_active(app) {
            render_simplified_content(frame, area, style);
            return Ok(());
        }
        
        // Render KV panel when subpage active
        let snap = extract_config_snapshot(app);
        render_kv_panel(frame, area, app, &snap);
        
        // Render simplified content fallback
        render_simplified_content(frame, area, style);
        Ok(())
    })?;
    
    Ok(())
}


