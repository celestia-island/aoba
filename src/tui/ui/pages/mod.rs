pub mod entry;
pub mod pull;
pub mod slave;

use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use crate::{i18n::lang, protocol::status::Status, tui::input::Action};

/// Return page-provided bottom hints for the current app state.
pub fn bottom_hints_for_app(app: &Status) -> Vec<String> {
    // If a subpage is active, delegate to it; otherwise use entry's hints.
    if let Some(sub) = app.active_subpage {
        match sub {
            crate::protocol::status::RightMode::Master => {
                return crate::tui::ui::pages::slave::page_bottom_hints(app)
            }
            crate::protocol::status::RightMode::SlaveStack => {
                return crate::tui::ui::pages::pull::page_bottom_hints(app)
            }
            crate::protocol::status::RightMode::Listen => {
                return crate::tui::ui::pages::entry::page_bottom_hints(app)
            }
        }
    }
    // default to entry hints when no subpage
    let mut hints = crate::tui::ui::pages::entry::page_bottom_hints(app);
    // If the transient mode selector overlay is active, append its hints so the bottom bar
    // can render them (keeps rendering centralized in bottom.rs)
    if app.mode_selector_active {
        hints.extend(crate::tui::ui::components::mode_selector::mode_selector_hints());
    }
    hints
}

/// Return global bottom hints that should appear on the bottom-most line regardless
/// of which subpage is active. This keeps page-specific hints separate (they can
/// be shown on an extra line above).
pub fn global_hints_for_app(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    // If a subpage is active, show back/list and tab-switch hints as global controls.
    if app.active_subpage.is_some() {
        hints.push(lang().hint_back_list.as_str().to_string());
        hints.push(lang().hint_switch_tab.as_str().to_string());
    } else {
        // default to entry hints when no subpage
        hints = crate::tui::ui::pages::entry::page_bottom_hints(app);
    }
    // If the transient mode selector overlay is active, append its hints so the bottom bar
    // can render them (keeps rendering centralized in bottom.rs)
    if app.mode_selector_active {
        hints.extend(crate::tui::ui::components::mode_selector::mode_selector_hints());
    }
    hints
}

/// Allow the active page to map a KeyEvent to a high-level Action when the global
/// key mapping returns no action. Returns Some(Action) if mapped.
pub fn map_key_in_page(key: KeyEvent, app: &Status) -> Option<Action> {
    if let Some(sub) = app.active_subpage {
        match sub {
            crate::protocol::status::RightMode::Master => {
                return crate::tui::ui::pages::slave::map_key(key, app)
            }
            crate::protocol::status::RightMode::SlaveStack => {
                return crate::tui::ui::pages::pull::map_key(key, app)
            }
            crate::protocol::status::RightMode::Listen => {
                return crate::tui::ui::pages::entry::map_key(key, app)
            }
        }
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

    if let Some(sub) = app.active_subpage {
        match sub {
            crate::protocol::status::RightMode::Master => {
                return slave::handle_subpage_key(key, app)
            }
            crate::protocol::status::RightMode::SlaveStack => {
                return pull::handle_subpage_key(key, app)
            }
            crate::protocol::status::RightMode::Listen => {
                return entry::handle_subpage_key(key, app)
            }
        }
    }
    false
}

pub fn render_panels(f: &mut Frame, area: Rect, app: &mut Status) {
    // If a subpage is active, render it full-screen; otherwise render the normal entry view
    if let Some(sub) = app.active_subpage {
        match sub {
            crate::protocol::status::RightMode::Master => slave::render_slave(f, area, app),
            crate::protocol::status::RightMode::SlaveStack => pull::render_pull(f, area, app),
            crate::protocol::status::RightMode::Listen => {
                // reuse entry's listen rendering but full area
                entry::render_entry(f, area, app)
            }
        }
    } else {
        entry::render_entry(f, area, app);
    }
}
