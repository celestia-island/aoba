use anyhow::{anyhow, ensure, Result};
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{self, Status},
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
pub fn spawn_input_thread(bus: Bus, app: Arc<RwLock<Status>>, thr_tx: flume::Sender<Result<()>>) {
    thread::spawn(move || {
        let res = (|| -> Result<()> {
            loop {
                // Poll for input. Keep this loop tight and avoid toggling mouse
                // capture here — constantly enabling/disabling mouse capture
                // interferes with terminal selection and adds latency.
                if let Ok(true) = crossterm::event::poll(Duration::from_millis(100)) {
                    if let Ok(ev) = crossterm::event::read() {
                        // handle_event now returns Result<()> and performs any quit
                        // signaling itself. Propagate errors, otherwise continue.
                        handle_event(ev, &bus, &app)?;
                    }
                }
            }
        })();
        let _ = thr_tx.send(res);
    });
}

/// Handle a single input event (keyboard or mouse)
fn handle_event(ev: crossterm::event::Event, bus: &Bus, app: &Arc<RwLock<Status>>) -> Result<()> {
    // Support both Key and Mouse scroll events. Map Mouse ScrollUp/Down to
    // synthesized KeyEvent Up/Down so existing key handlers can be reused.
    let mut key_opt: Option<KeyEvent> = None;

    match ev {
        crossterm::event::Event::Key(k) => {
            // Early catch for Ctrl+C at the top-level so the app can exit immediately.
            if k.kind == crossterm::event::KeyEventKind::Press
                && k.modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                && matches!(k.code, KeyCode::Char('c'))
            {
                bus.ui_tx.send(UiToCore::Quit).map_err(|e| anyhow!(e))?;
                return Ok(());
            }
            key_opt = Some(k);
        }
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
        handle_key_event(key, bus, app)?;
    }

    Ok(())
}

/// Handle a keyboard event
fn handle_key_event(key: KeyEvent, bus: &Bus, app: &Arc<RwLock<Status>>) -> Result<()> {
    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(()); // Ignore non-initial key press (repeat / release)
    }

    // Handle global quit with Ctrl+C
    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        if let KeyCode::Char('c') = key.code {
            bus.ui_tx.send(UiToCore::Quit).map_err(|e| anyhow!(e))?;
            return Ok(());
        }
    }

    // Route input to appropriate page handler based on current Status.page.
    if let Ok(snapshot) = read_status(app, |s| Ok(s.clone())) {
        use crate::tui::ui::pages;

        if matches!(snapshot.page, types::Page::About { .. }) {
            let snap_about = snapshot.snapshot_about();
            pages::about::handle_input(key, &snapshot, bus, app, &snap_about)?;
        } else if matches!(snapshot.page, types::Page::ModbusConfig { .. }) {
            let snap = snapshot.snapshot_modbus_config();
            pages::config_panel::handle_input(key, &snapshot, bus, app, &snap)?;
        } else if matches!(snapshot.page, types::Page::ModbusDashboard { .. }) {
            let snap = snapshot.snapshot_modbus_dashboard();
            pages::modbus_panel::input::handle_input(key, &snapshot, bus, app, &snap)?;
        } else if matches!(snapshot.page, types::Page::ModbusLog { .. }) {
            let snap = snapshot.snapshot_modbus_log();
            pages::log_panel::handle_input(key, &snapshot, bus, app, &snap)?;
        } else {
            // Default: ensure we are actually on Entry page before dispatching
            // This is an explicit boundary check requested by design to avoid
            // accidentally routing non-entry pages here.
            ensure!(
                matches!(snapshot.page, types::Page::Entry { .. }),
                "Expected Entry page in default branch"
            );

            let entry_snap = snapshot.snapshot_entry();
            pages::entry::handle_input(key, &snapshot, bus, app, &entry_snap)?;
        }
    }

    Ok(())
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
