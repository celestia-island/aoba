use anyhow::{anyhow, Result};
use std::time::Duration;

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
pub fn run_input_thread(bus: Bus, kill_rx: flume::Receiver<()>) -> Result<()> {
    loop {
        // Poll for input. Keep this loop tight and avoid toggling mouse
        // capture here — constantly enabling/disabling mouse capture
        // interferes with terminal selection and adds latency.
        if let Ok(true) = crossterm::event::poll(Duration::from_millis(100)) {
            if let Ok(event) = crossterm::event::read() {
                // handle_event now returns Result<()> and performs any quit
                // signaling itself. Propagate errors, otherwise continue.
                handle_event(event, &bus)?;
            }
        }

        // Check for kill signal to exit the input thread
        if let Ok(_) = kill_rx.try_recv() {
            break;
        }
    }

    Ok(())
}

fn handle_event(event: crossterm::event::Event, bus: &Bus) -> Result<()> {
    match event {
        crossterm::event::Event::Key(key) => {
            // Early catch for Ctrl + C at the top-level so the app can exit immediately.
            if key.kind == crossterm::event::KeyEventKind::Press
                && key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                bus.ui_tx.send(UiToCore::Quit).map_err(|err| anyhow!(err))?;
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

fn handle_key_event(key: KeyEvent, bus: &Bus) -> Result<()> {
    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(()); // Ignore non-initial key press (repeat / release)
    }

    // Handle global quit with Ctrl + C
    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        if let KeyCode::Char('c') = key.code {
            bus.ui_tx.send(UiToCore::Quit).map_err(|err| anyhow!(err))?;
            return Ok(());
        }
    }

    // Check if we're in global edit mode first
    if let Ok(snapshot) = read_status(|s| Ok(s.clone())) {
        // Check if any page is in edit mode
        let in_edit_mode = match &snapshot.page {
            types::Page::ConfigPanel { .. } => {
                // Check if we have an active edit cursor - simplified check
                !snapshot.temporarily.input_raw_buffer.is_empty()
                    || matches!(key.code, KeyCode::Enter)
            }
            _ => false,
        };

        // If in edit mode, handle character input globally
        if in_edit_mode && matches!(key.code, KeyCode::Char(_)) {
            if let KeyCode::Char(c) = key.code {
                use crate::protocol::status::write_status;
                write_status(|s| {
                    s.temporarily.input_raw_buffer.push(c);
                    Ok(())
                })?;
                bus.ui_tx
                    .send(UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                return Ok(());
            }
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
            types::Page::ConfigPanel { .. } => {
                pages::config_panel::handle_input(key, bus)?;
                return Ok(());
            }
            types::Page::ModbusDashboard { .. } => {
                pages::modbus_panel::input::handle_input(key, bus)?;
                return Ok(());
            }
            types::Page::LogPanel { .. } => {
                pages::log_panel::handle_input(key, bus)?;
                return Ok(());
            }
        }
    }

    Ok(())
}
