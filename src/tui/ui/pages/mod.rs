pub mod about;
pub mod config_panel;
pub mod entry;
pub mod log_panel;
pub mod modbus_panel;
pub mod mqtt_panel;

use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use crate::{i18n::lang, protocol::status::Status, tui::input::Action, tui::utils::bus::Bus};

/// Return page-provided bottom hints for the current app state.
pub fn bottom_hints_for_app(app: &Status) -> Vec<String> {
    if app.page.subpage_active {
        // If About full-page is active, let About provide page-specific hints
        let about_idx = app.ports.list.len().saturating_add(2);
        if app.page.selected == about_idx {
            return crate::tui::ui::pages::about::page_bottom_hints(app);
        }
        // Dispatch to the currently selected subpage tab (visual tabs).
        use crate::protocol::status::SubpageTab;
        match app.page.subpage_tab_index {
            SubpageTab::Config => {
                return crate::tui::ui::pages::config_panel::page_bottom_hints(app)
            }
            SubpageTab::Body => {
                // Body may be Modbus or MQTT depending on app mode
                match app.page.app_mode {
                    crate::protocol::status::AppMode::Modbus => {
                        return crate::tui::ui::pages::modbus_panel::page_bottom_hints(app)
                    }
                    crate::protocol::status::AppMode::Mqtt => {
                        return crate::tui::ui::pages::mqtt_panel::page_bottom_hints(app)
                    }
                }
            }
            SubpageTab::Log => return crate::tui::ui::pages::log_panel::page_bottom_hints(app),
        }
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
    if app.page.subpage_active {
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
    if app.page.subpage_active {
        use crate::protocol::status::SubpageTab;
        match app.page.subpage_tab_index {
            SubpageTab::Config => return crate::tui::ui::pages::config_panel::map_key(key, app),
            SubpageTab::Body => match app.page.app_mode {
                crate::protocol::status::AppMode::Modbus => {
                    return crate::tui::ui::pages::modbus_panel::map_key(key, app)
                }
                crate::protocol::status::AppMode::Mqtt => {
                    return crate::tui::ui::pages::mqtt_panel::map_key(key, app)
                }
            },
            SubpageTab::Log => return crate::tui::ui::pages::log_panel::map_key(key, app),
        }
    }
    crate::tui::ui::pages::entry::map_key(key, app)
}

/// Route a KeyEvent to the active subpage.
/// Returns true if the subpage consumed the event and no further handling should occur.
pub fn handle_key_in_subpage(key: KeyEvent, app: &mut Status, bus: &Bus) -> bool {
    use crossterm::event::KeyCode as KC;

    // Always let 'q' bubble up to the top-level quit handler (don't consume it here).
    if let KC::Char('q') | KC::Char('Q') = key.code {
        return false;
    }

    if app.page.subpage_active {
        // If About full-page is active, consume navigation keys here.
        let about_idx = app.ports.list.len().saturating_add(2);
        if app.page.selected == about_idx {
            match key.code {
                KC::Up | KC::Char('k') => {
                    app.ports.about_view_offset = app.ports.about_view_offset.saturating_sub(1);
                    return true;
                }
                KC::Down | KC::Char('j') => {
                    app.ports.about_view_offset = app.ports.about_view_offset.saturating_add(1);
                    return true;
                }
                KC::PageUp => {
                    app.ports.about_view_offset = app
                        .ports
                        .about_view_offset
                        .saturating_sub(crate::tui::utils::constants::LOG_PAGE_JUMP);
                    return true;
                }
                KC::PageDown => {
                    app.ports.about_view_offset = app
                        .ports
                        .about_view_offset
                        .saturating_add(crate::tui::utils::constants::LOG_PAGE_JUMP);
                    return true;
                }
                KC::Home => {
                    app.ports.about_view_offset = 0;
                    return true;
                }
                KC::End => {
                    app.ports.about_view_offset = 0;
                    return true;
                }
                _ => {}
            }
            // For other keys, do not consume so top-level can handle them (e.g., ESC to leave)
            return false;
        }
        use crate::protocol::status::SubpageTab;
        match app.page.subpage_tab_index {
            SubpageTab::Config => {
                return crate::tui::ui::pages::config_panel::handle_subpage_key(key, app, bus)
            }
            SubpageTab::Body => match app.page.app_mode {
                crate::protocol::status::AppMode::Modbus => {
                    return crate::tui::ui::pages::modbus_panel::handle_subpage_key(key, app, bus)
                }
                crate::protocol::status::AppMode::Mqtt => {
                    return crate::tui::ui::pages::mqtt_panel::handle_subpage_key(key, app, bus)
                }
            },
            SubpageTab::Log => {
                return crate::tui::ui::pages::log_panel::handle_subpage_key(key, app, bus)
            }
        }
    }
    false
}

pub fn render_panels(f: &mut Frame, area: Rect, app: &mut Status) {
    // If a subpage is active, render it full-screen; otherwise render the normal entry view
    if app.page.subpage_active {
        // If the current selection is the About virtual entry, render About full-screen
        let about_idx = app.ports.list.len().saturating_add(2);
        if app.page.selected == about_idx {
            crate::tui::ui::pages::about::render_about(f, area, app);
            return;
        }
        use crate::protocol::status::SubpageTab;
        match app.page.subpage_tab_index {
            SubpageTab::Config => {
                crate::tui::ui::pages::config_panel::render_config_panel(f, area, app, None)
            }
            SubpageTab::Body => match app.page.app_mode {
                crate::protocol::status::AppMode::Modbus => {
                    crate::tui::ui::pages::modbus_panel::render_modbus_panel(f, area, app)
                }
                crate::protocol::status::AppMode::Mqtt => {
                    crate::tui::ui::pages::mqtt_panel::render_mqtt_panel(f, area, app)
                }
            },
            SubpageTab::Log => crate::tui::ui::pages::log_panel::render_log_panel(f, area, app),
        }
    } else {
        entry::render_entry(f, area, app);
    }
}
