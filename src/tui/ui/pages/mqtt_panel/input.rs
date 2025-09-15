use crate::protocol::status::types::{self};
use crate::tui::utils::bus::Bus;
use anyhow::{anyhow, Result};

/// Handle input for MQTT panel. Sends commands via UiToCore.
pub fn handle_input(
    _key: crossterm::event::KeyEvent,
    bus: &Bus,
    _snap: &types::ui::EntryStatus,
) -> Result<bool> {
    use crossterm::event::KeyCode as KC;

    match _key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Basic navigation
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        // No direct Esc handling here; map_key returns LeavePage so routing layer will handle navigation.
        _ => Ok(false),
    }
}
