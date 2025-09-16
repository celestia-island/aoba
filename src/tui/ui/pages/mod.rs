pub mod about;
pub mod config_panel;
pub mod entry;
pub mod log_panel;
pub mod modbus_panel;
pub mod mqtt_panel;

use ratatui::prelude::*;

// AppMode and SubpageTab are not used directly in this module; derive from Page when needed
use crate::protocol::status::types::{self, Status};

// removed unused import `lang`

// Helper: derive the current selection index from `page` so callers
// don't rely on transient `temporarily.selected`.
fn derive_selection(app: &Status) -> usize {
    match &app.page {
        types::Page::Entry { cursor } => match cursor {
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            Some(types::ui::EntryCursor::About) => app.ports.order.len().saturating_add(2),
            Some(types::ui::EntryCursor::Refresh) => app.ports.order.len(),
            Some(types::ui::EntryCursor::CreateVirtual) => app.ports.order.len().saturating_add(1),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        _ => 0usize,
    }
}

/// Return page-provided bottom hints for the current app state.
/// Now returns a Vec of rows, where each row is a Vec of hint fragments.
pub fn bottom_hints_for_app(app: &Status) -> Vec<Vec<String>> {
    // Derive subpage activity and which tab from `page`.
    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    if subpage_active {
        // If About full-page is active, let About provide page-specific hints
        let about_idx = app.ports.order.len().saturating_add(2);
        let sel = derive_selection(app);
        if sel == about_idx {
            // Extract AboutStatus directly from app.page
            match &app.page {
                types::Page::About { view_offset } => {
                    let snap = types::ui::AboutStatus {
                        view_offset: *view_offset,
                    };
                    return about::page_bottom_hints(app, &snap);
                }
                other => panic!("Expected About page for bottom hints, found: {:?}", other),
            }
        }

        // Dispatch based on current_page variant. For ModbusDashboard/Config use Modbus body hints
        match app.page {
            types::Page::ModbusConfig { .. } => match &app.page {
                types::Page::ModbusConfig {
                    selected_port,
                    edit_active,
                    edit_port,
                    edit_field_index,
                    edit_field_key,
                    edit_buffer,
                    edit_cursor_pos,
                    ..
                } => {
                    let snap = types::ui::ModbusConfigStatus {
                        selected_port: *selected_port,
                        edit_active: *edit_active,
                        edit_port: edit_port.clone(),
                        edit_field_index: *edit_field_index,
                        edit_field_key: edit_field_key.clone(),
                        edit_buffer: edit_buffer.clone(),
                        edit_cursor_pos: *edit_cursor_pos,
                    };
                    return config_panel::page_bottom_hints(app, &snap);
                }
                other => panic!("Expected ModbusConfig for bottom hints, found: {:?}", other),
            },
            types::Page::ModbusDashboard { .. } => {
                match &app.page {
                    types::Page::ModbusDashboard { selected_port, .. } => {
                        // Construct a ModbusConfigStatus using dashboard's selected_port and defaults
                        let snap = types::ui::ModbusConfigStatus {
                            selected_port: *selected_port,
                            edit_active: false,
                            edit_port: None,
                            edit_field_index: 0,
                            edit_field_key: None,
                            edit_buffer: String::new(),
                            edit_cursor_pos: 0,
                        };
                        return config_panel::page_bottom_hints(app, &snap);
                    }
                    other => panic!(
                        "Expected ModbusDashboard for bottom hints, found: {:?}",
                        other
                    ),
                }
            }
            types::Page::ModbusLog { .. } => match &app.page {
                types::Page::ModbusLog { selected_port } => {
                    let snap = types::ui::ModbusLogStatus {
                        selected_port: *selected_port,
                    };
                    return log_panel::page_bottom_hints(app, &snap);
                }
                other => panic!(
                    "Expected ModbusLog page for bottom hints, found: {:?}",
                    other
                ),
            },
            _ => {}
        }
    }
    // Default to entry hints when no subpage
    match &app.page {
        types::Page::Entry { cursor } => {
            let entry_snap = types::ui::EntryStatus { cursor: *cursor };
            entry::page_bottom_hints(app, &entry_snap)
        }
        other => panic!("Expected Entry page for bottom hints, found: {:?}", other),
    }
}

/// Handle input for the currently active page (including entry when no subpage active).
/// Returns true if the page consumed the key event.
// Note: input dispatching has been centralized in `tui::input::handle_key_event`.
// Page modules only expose their individual handlers (entry::handle_input,
// about::handle_input, etc.).

/// Render the appropriate page based on the current app state.
/// This function only reads from Status and renders - no mutations allowed.
pub fn render_panels(f: &mut Frame, area: Rect, app: &Status) {
    // If a subpage is active, render it full-screen; otherwise render the normal entry view
    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    if subpage_active {
        // If the current selection is the About virtual entry, render About full-screen
        let about_idx = app.ports.order.len().saturating_add(2);
        let sel = derive_selection(app);
        if sel == about_idx {
            match &app.page {
                types::Page::About { view_offset } => {
                    let snap = types::ui::AboutStatus {
                        view_offset: *view_offset,
                    };
                    about::render(f, area, app, &snap);
                    return;
                }
                other => panic!("Expected About page for render, found: {:?}", other),
            }
        }
        match app.page {
            types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. } => {
                match &app.page {
                    types::Page::ModbusConfig {
                        selected_port,
                        edit_active,
                        edit_port,
                        edit_field_index,
                        edit_field_key,
                        edit_buffer,
                        edit_cursor_pos,
                        ..
                    } => {
                        let snap = types::ui::ModbusConfigStatus {
                            selected_port: *selected_port,
                            edit_active: *edit_active,
                            edit_port: edit_port.clone(),
                            edit_field_index: *edit_field_index,
                            edit_field_key: edit_field_key.clone(),
                            edit_buffer: edit_buffer.clone(),
                            edit_cursor_pos: *edit_cursor_pos,
                        };
                        config_panel::render(f, area, app, None, &snap)
                    }
                    types::Page::ModbusDashboard { selected_port, .. } => {
                        // Dashboard reuses config panel; build a ModbusConfigStatus with defaults
                        let snap = types::ui::ModbusConfigStatus {
                            selected_port: *selected_port,
                            edit_active: false,
                            edit_port: None,
                            edit_field_index: 0,
                            edit_field_key: None,
                            edit_buffer: String::new(),
                            edit_cursor_pos: 0,
                        };
                        config_panel::render(f, area, app, None, &snap)
                    }
                    other => panic!(
                        "Expected ModbusConfig or ModbusDashboard for render, found: {:?}",
                        other
                    ),
                }
            }
            types::Page::ModbusLog { .. } => match &app.page {
                types::Page::ModbusLog { selected_port } => {
                    let snap = types::ui::ModbusLogStatus {
                        selected_port: *selected_port,
                    };
                    log_panel::render(f, area, app, &snap)
                }
                other => panic!("Expected ModbusLog for render, found: {:?}", other),
            },
            types::Page::About { .. } => match &app.page {
                types::Page::About { view_offset } => {
                    let snap = types::ui::AboutStatus {
                        view_offset: *view_offset,
                    };
                    about::render(f, area, app, &snap)
                }
                other => panic!("Expected About for render, found: {:?}", other),
            },
            _ => match &app.page {
                types::Page::Entry { cursor } => {
                    let entry_snap = types::ui::EntryStatus { cursor: *cursor };
                    entry::render(f, area, app, &entry_snap)
                }
                other => panic!("Expected Entry for render, found: {:?}", other),
            },
        }
    } else {
        match &app.page {
            types::Page::Entry { cursor } => {
                let entry_snap = types::ui::EntryStatus { cursor: *cursor };
                entry::render(f, area, app, &entry_snap);
            }
            other => panic!(
                "Expected Entry page when rendering panels, found: {:?}",
                other
            ),
        }
    }
}
