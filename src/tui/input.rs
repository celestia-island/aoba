use crossterm::event::{KeyCode, KeyEvent};
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use crate::{
    protocol::status::{
        types::{self, ui::SpecialEntry, Status},
        write_status,
    },
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
        KeyCode::Esc | KeyCode::Char('h') => Action::LeavePage,
        // Key 'l' used to enter subpage historically; removed per UI change.
        // Map Enter to EnterPage instead.
        KeyCode::Down | KeyCode::Char('j') => Action::MoveNext,
        KeyCode::Up | KeyCode::Char('k') => Action::MovePrev,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::JumpTop,
        KeyCode::End => Action::JumpBottom,
        KeyCode::Char('c') => Action::ClearError,
        KeyCode::Char('e') => Action::EditToggle,
        KeyCode::Char('n') => Action::AddRegister,
        KeyCode::Char('d') => Action::DeleteRegister,
        KeyCode::Char('p') => Action::ToggleFollow,
        KeyCode::Char('r') => Action::QuickScan,
        KeyCode::Tab => Action::SwitchNext,
        KeyCode::BackTab => Action::SwitchPrev,
        KeyCode::Enter => Action::EnterPage,
        // Space toggles runtime/selection (was Enter previously)
        KeyCode::Char(' ') => Action::TogglePort,

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
                    consumed_by_page = about::handle_mouse(
                        me,
                        &snapshot,
                        bus,
                        app,
                        &snap_about,
                    );
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
    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        if let KeyCode::Char('c') = key.code {
            let _ = bus.ui_tx.send(UiToCore::Quit);
            return true;
        }
    }

    // Route input to appropriate page handler
    if let Ok(snapshot) = crate::protocol::status::read_status(app, |s| Ok(s.clone())) {
        // First, let subpages consume input if applicable
        let consumed = crate::tui::ui::pages::handle_input_in_subpage(
            key, &snapshot, bus, app,
        );

        if !consumed {
            // Try to handle as a page-specific action first
            if let Some(page_action) = crate::tui::ui::pages::map_key_in_page(key, &snapshot) {
                handle_page_action(page_action, bus, app, &snapshot);
            } else {
                // Handle global keys
                let action = map_key(key.code);
                handle_global_action(action, bus, app, &snapshot);
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
        Action::MoveNext | Action::MovePrev => {
            // These are still handled globally for now, but could be moved to pages
            handle_global_action(action, bus, app, snapshot);
        }
        _ => {
            // Let the page handle its own actions via handle_input_in_page
            let _ = crate::tui::ui::pages::handle_input_in_page(
                KeyEvent::new(KeyCode::Null, crossterm::event::KeyModifiers::NONE), // placeholder
                snapshot, bus, app,
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
        Action::MoveNext => {
            handle_move_next(bus, app);
        }
        Action::MovePrev => {
            handle_move_prev(bus, app);
        }
        Action::EnterPage => {
            handle_enter_page(bus, app, snapshot);
        }
        Action::LeavePage => {
            handle_leave_page(bus, app);
        }
        Action::TogglePort => {
            handle_toggle_port(bus, app, snapshot);
        }
        Action::QuickScan => {
            let _ = bus.ui_tx.send(UiToCore::Refresh);
        }
        Action::None => {
            // If global mapping didn't handle it, try page-level handlers
            let _ = crate::tui::ui::pages::handle_input_in_page(
                KeyEvent::new(KeyCode::Null, crossterm::event::KeyModifiers::NONE),
                snapshot, bus, app,
            );
        }
        _ => {
            // Other actions can be handled by pages
        }
    }
}

/// Handle moving selection down
fn handle_move_next(bus: &Bus, app: &Arc<RwLock<Status>>) {
    let _ = write_status(app, |s| {
        let special_base = s.ports.order.len();
        let extra_count = SpecialEntry::all().len();
        let total = special_base + extra_count;
        
        let mut sel = derive_selection_from_page(&s.page, &s.ports.order);
        if sel + 1 < total {
            sel += 1;
        } else {
            sel = total.saturating_sub(1);
        }
        
        // Write back as Entry cursor
        s.page = types::Page::Entry {
            cursor: Some(types::ui::EntryCursor::Com { idx: sel }),
        };
        Ok(())
    });
    let _ = bus.ui_tx.send(UiToCore::Refresh);
}

/// Handle moving selection up
fn handle_move_prev(bus: &Bus, app: &Arc<RwLock<Status>>) {
    let _ = write_status(app, |s| {
        let mut sel = derive_selection_from_page(&s.page, &s.ports.order);
        if sel > 0 {
            sel = sel.saturating_sub(1);
        } else {
            sel = 0;
        }
        
        s.page = types::Page::Entry {
            cursor: Some(types::ui::EntryCursor::Com { idx: sel }),
        };
        Ok(())
    });
    let _ = bus.ui_tx.send(UiToCore::Refresh);
}

/// Handle entering a page/subpage
fn handle_enter_page(bus: &Bus, app: &Arc<RwLock<Status>>, _snapshot: &Status) {
    if let Ok((sel, ports_order)) = crate::protocol::status::read_status(app, |s| {
        let sel = derive_selection_from_page(&s.page, &s.ports.order);
        Ok((sel, s.ports.order.clone()))
    }) {
        let ports_len = ports_order.len();
        if sel < ports_len {
            // Open ModbusDashboard for the selected port
            let port_name = ports_order.get(sel).cloned().unwrap_or_default();
            let _ = write_status(app, |s| {
                s.page = types::Page::ModbusDashboard {
                    selected_port: sel,
                    cursor: 0,
                    editing_field: None,
                    input_buffer: String::new(),
                    edit_choice_index: None,
                    edit_confirmed: false,
                    master_cursor: 0,
                    master_field_selected: false,
                    master_field_editing: false,
                    master_edit_field: None,
                    master_edit_index: None,
                    master_input_buffer: String::new(),
                    poll_round_index: 0,
                    in_flight_reg_index: None,
                };
                s.temporarily.per_port.pending_sync_port = Some(port_name.clone());
                Ok(())
            });
            let _ = bus.ui_tx.send(UiToCore::Refresh);
        } else {
            // Selection points into special entries (Refresh, ManualSpecify, About)
            let rel = sel.saturating_sub(ports_len);
            // If About (third special entry) is selected -> open About page
            if rel == 2 {
                let _ = write_status(app, |s| {
                    s.page = types::Page::About { view_offset: 0 };
                    Ok(())
                });
                let _ = bus.ui_tx.send(UiToCore::Refresh);
            }
        }
    }
}

/// Handle leaving current page/subpage
fn handle_leave_page(bus: &Bus, app: &Arc<RwLock<Status>>) {
    let _ = write_status(app, |s| {
        // Only change page when currently in a subpage
        let subpage_active = matches!(
            s.page,
            types::Page::ModbusConfig { .. }
                | types::Page::ModbusDashboard { .. }
                | types::Page::ModbusLog { .. }
                | types::Page::About { .. }
        );
        if subpage_active {
            s.page = types::Page::Entry { cursor: None };
        }
        Ok(())
    });
    let _ = bus.ui_tx.send(UiToCore::Refresh);
}

/// Handle toggling port runtime
fn handle_toggle_port(bus: &Bus, app: &Arc<RwLock<Status>>, _snapshot: &Status) {
    if let Ok((sel, ports_order)) = crate::protocol::status::read_status(app, |s| {
        let sel = derive_selection_from_page(&s.page, &s.ports.order);
        Ok((sel, s.ports.order.clone()))
    }) {
        let ports_len = ports_order.len();
        if sel < ports_len {
            if let Some(port_name) = ports_order.get(sel).cloned() {
                let _ = bus.ui_tx.send(UiToCore::ToggleRuntime(port_name));
            }
        }
    }
}

/// Helper function to derive selection from page state
fn derive_selection_from_page(page: &types::Page, ports_order: &[String]) -> usize {
    match page {
        types::Page::Entry { cursor } => match cursor {
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            Some(types::ui::EntryCursor::Refresh) => ports_order.len(),
            Some(types::ui::EntryCursor::CreateVirtual) => ports_order.len().saturating_add(1),
            Some(types::ui::EntryCursor::About) => ports_order.len().saturating_add(2),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        _ => 0usize,
    }
}
