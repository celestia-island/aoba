use anyhow::{anyhow, Result};
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{read_status, types, write_status},
    tui::{
        ui::pages,
        utils::bus::{Bus, UiToCore},
    },
};

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
        if kill_rx.try_recv().is_ok() {
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
        crossterm::event::Event::Mouse(event) => {
            match read_status(|status| Ok(status.page.clone())) {
                Ok(types::Page::Entry { .. }) => {
                    pages::entry::input::handle_mouse(event, bus)?;
                }
                Ok(types::Page::About { .. }) => {
                    pages::about::handle_mouse(event, bus)?;
                }
                _ => {}
            }
        }
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
    if let Ok((page, input_buffer)) = read_status(|status| {
        Ok((
            status.page.clone(),
            status.temporarily.input_raw_buffer.clone(),
        ))
    }) {
        // Check if any page is in edit mode
        let in_edit_mode = match &page {
            types::Page::ConfigPanel { .. } => {
                // Check if we have an active edit cursor - simplified check
                !input_buffer.is_empty() || matches!(key.code, KeyCode::Enter)
            }
            _ => false,
        };

        // If in edit mode, handle character input globally
        if in_edit_mode && matches!(key.code, KeyCode::Char(_)) {
            if let KeyCode::Char(c) = key.code {
                write_status(|status| {
                    status.temporarily.input_raw_buffer.push(c);
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
    if let Ok(page) = read_status(|status| Ok(status.page.clone())) {
        // Route by exact page variant and construct the page snapshot inline.
        match &page {
            types::Page::Entry { .. } => {
                pages::entry::handle_input(key, bus)?;
            }
            types::Page::About { .. } => {
                pages::about::handle_input(key, bus)?;
            }
            types::Page::ConfigPanel { .. } => {
                pages::config_panel::handle_input(key, bus)?;
            }
            types::Page::ModbusDashboard { .. } => {
                pages::modbus_panel::input::handle_input(key, bus)?;
            }
            types::Page::LogPanel { .. } => {
                pages::log_panel::handle_input(key, bus)?;
            }
        }
        return Ok(());
    }

    Ok(())
}
