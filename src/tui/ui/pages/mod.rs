pub mod about;
pub mod config_panel;
pub mod entry;
pub mod log_panel;
pub mod modbus_panel;
pub mod mqtt_panel;

use std::sync::{Arc, RwLock};

use crossterm::event::KeyEvent;
use ratatui::prelude::*;

// AppMode and SubpageTab are not used directly in this module; derive from Page when needed
use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
    tui::input::Action,
    tui::utils::bus::Bus,
};

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
pub fn bottom_hints_for_app(app: &Status) -> Vec<String> {
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
    let mut hints = entry::page_bottom_hints(app, &entry_snap);
    hints.push(lang().hotkeys.press_m_switch_protocol.as_str().to_string());
    hints
}

/// Return global bottom hints that should appear on the bottom-most line regardless
/// Of which subpage is active. This keeps page-specific hints separate (they can
/// Be shown on an extra line above).
pub fn global_hints_for_app(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    // If a subpage is active, show back / list and tab-switch hints as global controls.
    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    if subpage_active {
        hints.push(lang().hotkeys.hint_back_list.as_str().to_string());
        hints.push(lang().hotkeys.hint_switch_tab.as_str().to_string());
    } else {
        // Default to entry hints when no subpage
        let entry_snap = app.snapshot_entry();
        hints = entry::page_bottom_hints(app, &entry_snap);
        hints.push(lang().hotkeys.press_m_switch_protocol.as_str().to_string());
    }
    // If the transient mode selector overlay is active, append its hints so the bottom bar
    // Can render them (keeps rendering centralized in bottom.rs)
    // Mode selector hints now rendered inside popup; do not append here.
    hints
}

/// Allow the active page to map a KeyEvent to a high-level Action when the global
/// Key mapping returns no action. Returns Some(Action) if mapped.
pub fn map_key_in_page(key: KeyEvent, app: &Status) -> Option<Action> {
    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    if subpage_active {
        match app.page {
            types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. } => {
                let snap = app.snapshot_modbus_config();
                return config_panel::map_key(key, app, &snap);
            }
            types::Page::ModbusLog { .. } => {
                let snap = app.snapshot_modbus_log();
                return log_panel::map_key(key, app, &snap);
            }
            _ => {}
        }
    }
    let entry_snap = app.snapshot_entry();
    entry::map_key(key, app, &entry_snap)
}

/// Route a KeyEvent to the active subpage input handler.
/// Returns true if the subpage consumed the event and no further handling should occur.
pub fn handle_input_in_subpage(
    key: KeyEvent,
    app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
) -> bool {
    use crossterm::event::KeyCode as KC;

    // Always let 'q' bubble up to the top-level quit handler (don't consume it here).
    if let KC::Char('q') | KC::Char('Q') = key.code {
        return false;
    }

    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    if subpage_active {
        // If About full-page is active, consume navigation keys here.
        let about_idx = app.ports.order.len().saturating_add(2);
        let sel = derive_selection(app);
        if sel == about_idx {
            let snap = app.snapshot_about();
            return about::handle_input(key, app, bus, app_arc, &snap);
        }
        match app.page {
            types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. } => {
                let snap = app.snapshot_modbus_config();
                return config_panel::handle_input(key, app, bus, app_arc, &snap);
            }
            types::Page::ModbusLog { .. } => {
                let snap = app.snapshot_modbus_log();
                return log_panel::handle_input(key, bus, &snap);
            }
            _ => {}
        }
    }
    false
}

/// Handle input for the currently active page (including entry when no subpage active).
/// Returns true if the page consumed the key event.
pub fn handle_input_in_page(
    key: KeyEvent,
    app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
) -> bool {
    // If a subpage is active, delegate to existing subpage handlers (which already
    // consume keys like 'q' for About etc.). When no subpage is active, delegate to
    // the entry page raw handler so it can process non-global keys.
    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    if subpage_active {
        // Reuse existing subpage input routing which handles About specially.
        return handle_input_in_subpage(key, app, bus, app_arc);
    }

    // No subpage active: let entry page have a chance to consume the key.
    let entry_snap = app.snapshot_entry();
    entry::handle_input(key, app, bus, app_arc, &entry_snap)
}

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
