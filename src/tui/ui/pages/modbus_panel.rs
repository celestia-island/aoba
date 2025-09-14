use std::cmp::min;

use ratatui::{prelude::*, text::Line};

use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
    tui::ui::components::render_boxed_paragraph,
    tui::utils::bus::Bus,
};

/// Render the ModBus panel. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status, _snap: &types::ui::ModbusDashboardStatus) {
    let mut lines: Vec<Line> = Vec::new();

    // Simple display of ModBus status
    lines.push(Line::from("ModBus Panel"));
    lines.push(Line::from(""));

    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. }
    );
    if subpage_active {
        lines.push(Line::from(
            "Subpage form present (details moved to UI layer)",
        ));
    } else {
        lines.push(Line::from("No form data available"));
    }

    // Calculate visible area for scrolling
    let inner_height = area.height.saturating_sub(2) as usize;
    // Core no longer stores SubpageForm; default cursor to 0 for rendering purposes.
    let cursor_line = 0;

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

pub fn page_bottom_hints(_app: &Status, _snap: &types::ui::ModbusDashboardStatus) -> Vec<String> {
    let hints: Vec<String> = vec![
        lang().hotkeys.hint_move_vertical.as_str().to_string(),
        "Enter: Edit".to_string(),
        "Del: Delete".to_string(),
    ];
    hints
}

/// Handle input for ModBus panel. Sends commands via UiToCore.
pub fn handle_input(
    key: crossterm::event::KeyEvent,
    _app: &Status,
    bus: &Bus,
    app_arc: &std::sync::Arc<std::sync::RwLock<types::Status>>,
    _snap: &types::ui::ModbusDashboardStatus,
) -> bool {
    use crossterm::event::KeyCode as KC;

    match key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Navigation within the dashboard
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Left | KC::Right => {
            // Horizontal navigation within fields
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Esc => {
            // If dashboard has nested edit state in Status (e.g. editing_field or master_field_editing),
            // prefer to cancel those first. Otherwise leave to entry page.
            use crate::protocol::status::write_status;
            let mut cancelled = false;
            let _ = write_status(app_arc, |s| {
                if let types::Page::ModbusDashboard {
                    editing_field,
                    master_field_editing,
                    master_edit_field,
                    ..
                } = &mut s.page
                {
                    // If any editing sub-state is active, clear it instead of leaving page
                    if editing_field.is_some() || *master_field_editing {
                        *editing_field = None;
                        *master_field_editing = false;
                        *master_edit_field = None;
                        cancelled = true;
                    }
                }
                Ok(())
            });
            if cancelled {
                let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
                true
            } else {
                // No nested edit active: leave dashboard
                handle_leave_page(bus, app_arc);
                true
            }
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
        _ => false,
    }
}

/// Handle leaving the modbus dashboard back to entry page
fn handle_leave_page(bus: &Bus, app_arc: &std::sync::Arc<std::sync::RwLock<types::Status>>) {
    use crate::protocol::status::write_status;
    use crate::tui::utils::bus::UiToCore;

    let _ = write_status(app_arc, |s| {
        // Go back to entry page
        s.page = types::Page::Entry { cursor: None };
        Ok(())
    });
    let _ = bus.ui_tx.send(UiToCore::Refresh);
}
