use std::cmp::min;

use ratatui::{
    prelude::*,
    style::{Color, Style},
    text::Line,
};

use crate::tui::utils::bus::Bus;
use crate::{
    i18n::lang,
    protocol::status::{Status},
    tui::ui::components::{render_boxed_paragraph},
};

/// Render the ModBus panel. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status) {
    let mut lines: Vec<Line> = Vec::new();
    
    // Simple display of ModBus status
    lines.push(Line::from("ModBus Panel"));
    lines.push(Line::from(""));
    
    if let Some(ref form) = app.page.subpage_form {
        lines.push(Line::from(format!("Registers: {}", form.registers.len())));
        lines.push(Line::from(format!("Cursor: {}", form.master_cursor)));
        
        // Display some register info
        for (i, reg) in form.registers.iter().enumerate().take(10) {
            let prefix = if i == form.master_cursor { "> " } else { "  " };
            lines.push(Line::from(format!(
                "{}[{}] Role: {:?} Type: {:?}",
                prefix, i, reg.role, reg.reg_type
            )));
        }
        
        if form.registers.len() > 10 {
            lines.push(Line::from(format!("... and {} more", form.registers.len() - 10)));
        }
    } else {
        lines.push(Line::from("No form data available"));
    }

    // Calculate visible area for scrolling
    let inner_height = area.height.saturating_sub(2) as usize;
    let cursor_line = app.page.subpage_form
        .as_ref()
        .map(|f| f.master_cursor)
        .unwrap_or(0);
    
    let mut first_visible = 0;
    if cursor_line >= inner_height {
        first_visible = cursor_line + 1 - inner_height;
    }
    
    let total = lines.len();
    let last_start = total.saturating_sub(inner_height);
    if first_visible > last_start {
        first_visible = last_start;
    }
    let end = min(total, first_visible + inner_height);
    
    render_boxed_paragraph(f, area, lines[first_visible..end].to_vec(), None);
}

pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
    hints.push("Enter: Edit".to_string());
    hints.push("Del: Delete".to_string());
    hints
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
}

/// Handle input for ModBus panel. Sends commands via UiToCore.
pub fn handle_input(_key: crossterm::event::KeyEvent, bus: &Bus) -> bool {
    use crossterm::event::KeyCode as KC;

    match _key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Navigation
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Left | KC::Right | KC::Char('h') | KC::Char('l') => {
            // Horizontal navigation
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Enter => {
            // Edit entry
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Delete | KC::Char('x') => {
            // Delete entry
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Char('n') => {
            // New entry
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Tab => {
            // Tab switching
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false
    }
}