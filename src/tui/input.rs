use crossterm::event::{KeyCode, KeyEvent};
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use crate::{
    protocol::status::types::{self, Status},
    tui::{
        ui::pages::about,
        utils::bus::{Bus, UiToCore},
    },
};

/// High-level user actions
#[derive(Debug, Clone)]
pub enum Action {
    Quit,
    LeavePage,
    EnterPage,
    EditToggle,
    AddRegister,
    DeleteRegister,
    PageUp,
    PageDown,
    JumpTop,
    JumpBottom,
    MoveNext,
    MovePrev,
    ClearError,
    EnterSubpage(char),
    ExitSubpage,
    SwitchNext,
    SwitchPrev,
    TogglePort,
    ToggleFollow,
    QuickScan,
    None,
}

/// Map a KeyCode to a high-level Action
pub fn map_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Char('q') => Action::Quit,
        // Most navigation and page actions are now handled by individual pages
        KeyCode::Char('c') => Action::ClearError,
        KeyCode::Char('e') => Action::EditToggle,
        KeyCode::Char('n') => Action::AddRegister,
        KeyCode::Char('d') => Action::DeleteRegister,
        KeyCode::Char('p') => Action::ToggleFollow,
        KeyCode::Tab => Action::SwitchNext,
        KeyCode::BackTab => Action::SwitchPrev,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::JumpTop,
        KeyCode::End => Action::JumpBottom,

        _ => Action::None,
    }
}

/// Spawn the input handling thread that processes keyboard and mouse events
pub fn spawn_input_thread(bus: Bus, app: Arc<RwLock<Status>>) {
    thread::spawn(move || {
        loop {
            // Poll for input
            if let Ok(true) = crossterm::event::poll(Duration::from_millis(100)) {
                if let Ok(ev) = crossterm::event::read() {
                    if handle_event(ev, &bus, &app) {
                        break; // Quit was requested
                    }
                }
            }
        }
    });
}

/// Handle a single input event (keyboard or mouse)
/// Returns true if quit was requested
fn handle_event(ev: crossterm::event::Event, bus: &Bus, app: &Arc<RwLock<Status>>) -> bool {
    // Support both Key and Mouse scroll events. Map Mouse ScrollUp/Down to
    // synthesized KeyEvent Up/Down so existing key handlers can be reused.
    let mut key_opt: Option<KeyEvent> = None;

    match ev {
        crossterm::event::Event::Key(k) => key_opt = Some(k),
        crossterm::event::Event::Mouse(me) => {
            // First: if About page is active, let it consume the mouse scroll.
            let mut consumed_by_page = false;
            if let Ok(snapshot) = crate::protocol::status::read_status(app, |s| Ok(s.clone())) {
                // Check if About is active (either selected virtual entry or full page)
                let about_idx = snapshot.ports.order.len().saturating_add(2);
                let sel = derive_selection(&snapshot);

                if sel == about_idx {
                    // Snapshot for about page input
                    let snap_about = snapshot.snapshot_about();
                    consumed_by_page = about::handle_mouse(me, &snapshot, bus, app, &snap_about);
                }
            }

            if consumed_by_page {
                // Page consumed the mouse event; do not map to a key
            } else {
                // Fallback: map scroll to Up/Down key events for global handling
                use crossterm::event::MouseEventKind as MEK;
                match me.kind {
                    MEK::ScrollUp => {
                        key_opt = Some(KeyEvent::new(
                            KeyCode::Up,
                            crossterm::event::KeyModifiers::NONE,
                        ));
                    }
                    MEK::ScrollDown => {
                        key_opt = Some(KeyEvent::new(
                            KeyCode::Down,
                            crossterm::event::KeyModifiers::NONE,
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    if let Some(key) = key_opt {
        return handle_key_event(key, bus, app);
    }

    false
}

/// Handle a keyboard event
/// Returns true if quit was requested
fn handle_key_event(key: KeyEvent, bus: &Bus, app: &Arc<RwLock<Status>>) -> bool {
    if key.kind != crossterm::event::KeyEventKind::Press {
        return false; // Ignore non-initial key press (repeat / release)
    }

    // Handle global quit with Ctrl+C
    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        if let KeyCode::Char('c') = key.code {
            let _ = bus.ui_tx.send(UiToCore::Quit);
            return true;
        }
    }

    // Route input to appropriate page handler
    if let Ok(snapshot) = crate::protocol::status::read_status(app, |s| Ok(s.clone())) {
        // First, let subpages consume input if applicable
        let consumed = crate::tui::ui::pages::handle_input_in_subpage(key, &snapshot, bus, app);

        if !consumed {
            // If the active page maps the key, dispatch the original KeyEvent to the page handler
            if crate::tui::ui::pages::map_key_in_page(key, &snapshot).is_some() {
                let _ = crate::tui::ui::pages::handle_input_in_page(key, &snapshot, bus, app);
            } else {
                let action = map_key(key.code);
                if let Action::None = action {
                    let _ = crate::tui::ui::pages::handle_input_in_page(key, &snapshot, bus, app);
                } else {
                    handle_global_action(action, bus, app, &snapshot);
                }
            }
        }
    }

    false
}

/// Derive the current selection index from the page state
fn derive_selection(app: &Status) -> usize {
    match &app.page {
        types::Page::Entry { cursor } => match cursor {
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            Some(types::ui::EntryCursor::Refresh) => app.ports.order.len(),
            Some(types::ui::EntryCursor::CreateVirtual) => app.ports.order.len().saturating_add(1),
            Some(types::ui::EntryCursor::About) => app.ports.order.len().saturating_add(2),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        types::Page::About { .. } => app.ports.order.len().saturating_add(2),
    }
}

/// Handle actions that are mapped by individual pages
fn handle_page_action(action: Action, bus: &Bus, app: &Arc<RwLock<Status>>, snapshot: &Status) {
    match action {
        _ => {
            // All actions are now handled by pages themselves
            // The global input handler just routes to the appropriate page
            let _ = crate::tui::ui::pages::handle_input_in_page(
                KeyEvent::new(KeyCode::Null, crossterm::event::KeyModifiers::NONE), // placeholder
                snapshot,
                bus,
                app,
            );
        }
    }
}

/// Handle global actions that affect the entire application
fn handle_global_action(action: Action, bus: &Bus, app: &Arc<RwLock<Status>>, snapshot: &Status) {
    match action {
        Action::Quit => {
            let _ = bus.ui_tx.send(UiToCore::Quit);
        }
        Action::None => {
            // If global mapping didn't handle it, try page-level handlers
            let _ = crate::tui::ui::pages::handle_input_in_page(
                KeyEvent::new(KeyCode::Null, crossterm::event::KeyModifiers::NONE),
                snapshot,
                bus,
                app,
            );
        }
        _ => {
            // Most actions are now handled by individual pages
            // If we reach here, it means the page didn't consume the action
            // This is expected for page-specific actions
        }
    }
}
