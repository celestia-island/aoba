use anyhow::{anyhow, Result};
use std::{thread, time::Duration};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{read_status, types},
    tui::{
        ui::pages,
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
pub fn spawn_input_thread(bus: Bus, thr_tx: flume::Sender<Result<()>>) {
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
                        handle_event(ev, &bus)?;
                    }
                }
            }
        })();
        let _ = thr_tx.send(res);
    });
}

/// Handle a single input event (keyboard or mouse)
fn handle_event(ev: crossterm::event::Event, bus: &Bus) -> Result<()> {
    // Support both Key and Mouse scroll events. Map Mouse ScrollUp/Down to
    // synthesized KeyEvent Up/Down so existing key handlers can be reused.

    match ev {
        crossterm::event::Event::Key(key) => {
            // Early catch for Ctrl+C at the top-level so the app can exit immediately.
            if key.kind == crossterm::event::KeyEventKind::Press
                && key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                bus.ui_tx.send(UiToCore::Quit).map_err(|e| anyhow!(e))?;
                return Ok(());
            }

            handle_key_event(key, bus)?;
        }
        crossterm::event::Event::Mouse(event) => match read_status(|s| Ok(s.page.clone())) {
            Ok(types::Page::Entry { .. }) => {
                pages::entry::input::handle_mouse(event, bus)?;
            }
            Ok(types::Page::About { .. }) => {
                pages::about::handle_mouse(event, bus)?;
            }
            _ => {}
        },
        _ => {}
    }

    Ok(())
}

/// Handle a keyboard event
fn handle_key_event(key: KeyEvent, bus: &Bus) -> Result<()> {
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
    if let Ok(snapshot) = read_status(|s| Ok(s.clone())) {
        use crate::tui::ui::pages;

        // Route by exact page variant and construct the page snapshot inline.
        match &snapshot.page {
            types::Page::Entry { .. } => {
                pages::entry::handle_input(key, bus)?;
                return Ok(());
            }
            types::Page::About { .. } => {
                pages::about::handle_input(key, bus)?;
                return Ok(());
            }
            types::Page::ModbusConfig { .. } => {
                pages::config_panel::handle_input(key, bus)?;
                return Ok(());
            }
            types::Page::ModbusDashboard { .. } => {
                pages::modbus_panel::input::handle_input(key, bus)?;
                return Ok(());
            }
            types::Page::ModbusLog { .. } => {
                pages::log_panel::handle_input(key, bus)?;
                return Ok(());
            }
        }
    }

    Ok(())
}
