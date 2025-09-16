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
            let snap = app.snapshot_about();
            return about::page_bottom_hints(app, &snap);
        }

        // Dispatch based on current_page variant. For ModbusDashboard/Config use Modbus body hints
        match app.page {
            types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. } => {
                let snap = app.snapshot_modbus_config();
                return config_panel::page_bottom_hints(app, &snap);
            }
            types::Page::ModbusLog { .. } => {
                let snap = app.snapshot_modbus_log();
                return log_panel::page_bottom_hints(app, &snap);
            }
            types::Page::About { .. } => {
                let snap = app.snapshot_about();
                return about::page_bottom_hints(app, &snap);
            }
            _ => {}
        }
    }
    // Default to entry hints when no subpage
    let entry_snap = app.snapshot_entry();
    entry::page_bottom_hints(app, &entry_snap)
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
            let snap = app.snapshot_about();
            about::render(f, area, app, &snap);
            return;
        }
        match app.page {
            types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. } => {
                let snap = app.snapshot_modbus_config();
                config_panel::render(f, area, app, None, &snap)
            }
            types::Page::ModbusLog { .. } => {
                let snap = app.snapshot_modbus_log();
                log_panel::render(f, area, app, &snap)
            }
            types::Page::About { .. } => {
                let snap = app.snapshot_about();
                about::render(f, area, app, &snap)
            }
            _ => {
                let entry_snap = app.snapshot_entry();
                entry::render(f, area, app, &entry_snap)
            }
        }
    } else {
        let entry_snap = app.snapshot_entry();
        entry::render(f, area, app, &entry_snap);
    }
}
