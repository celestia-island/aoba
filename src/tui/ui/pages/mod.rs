pub mod entry;
pub mod modbus;

use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use crate::{i18n::lang, protocol::status::Status, tui::input::Action};

/// Return page-provided bottom hints for the current app state.
pub fn bottom_hints_for_app(app: &Status) -> Vec<String> {
    if app.subpage_active {
        return crate::tui::ui::pages::modbus::page_bottom_hints(app);
    }
    // Default to entry hints when no subpage
    let mut hints = crate::tui::ui::pages::entry::page_bottom_hints(app);
    hints.push(lang().hotkeys.press_m_switch_protocol.as_str().to_string());
    hints
}

/// Return global bottom hints that should appear on the bottom-most line regardless
/// Of which subpage is active. This keeps page-specific hints separate (they can
/// Be shown on an extra line above).
pub fn global_hints_for_app(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    // If a subpage is active, show back / list and tab-switch hints as global controls.
    if app.subpage_active {
        hints.push(lang().hotkeys.hint_back_list.as_str().to_string());
        hints.push(lang().hotkeys.hint_switch_tab.as_str().to_string());
    } else {
        // Default to entry hints when no subpage
        hints = crate::tui::ui::pages::entry::page_bottom_hints(app);
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
    if app.subpage_active {
        return crate::tui::ui::pages::modbus::map_key(key, app);
    }
    crate::tui::ui::pages::entry::map_key(key, app)
}

/// Route a KeyEvent to the active subpage.
/// Returns true if the subpage consumed the event and no further handling should occur.
pub fn handle_key_in_subpage(key: KeyEvent, app: &mut Status) -> bool {
    use crossterm::event::KeyCode as KC;

    // Always let 'q' bubble up to the top-level quit handler (don't consume it here).
    if let KC::Char('q') | KC::Char('Q') = key.code {
        return false;
    }

    if app.subpage_active {
        return modbus::handle_subpage_key(key, app);
    }
    false
}

pub fn render_panels(f: &mut Frame, area: Rect, app: &mut Status) {
    // If a subpage is active, render it full-screen; otherwise render the normal entry view
    if app.subpage_active {
        modbus::render_modbus(f, area, app);
    } else {
        entry::render_entry(f, area, app);
    }
}
