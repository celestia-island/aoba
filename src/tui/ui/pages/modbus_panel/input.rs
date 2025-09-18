use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{protocol::status::types, tui::utils::bus::Bus};

/// Handle input for ModBus panel. Sends commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
            // Navigation within the dashboard
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Left | KeyCode::Right => {
            // Horizontal navigation within fields
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Esc => {
            // If dashboard has nested edit state in Status (e.g. editing_field or master_field_editing),
            // prefer to cancel those first. Otherwise leave to entry page.
            use crate::protocol::status::write_status;
            let mut cancelled = false;
            let _ = write_status(|s| {
                if let types::Page::ModbusDashboard {
                    editing_field,
                    master_field_editing,
                    master_edit_field,
                    ..
                } = &mut s.page
                {
                    // If any editing sub-state is active, clear it instead of leaving page
                    if editing_field.is_some() || *master_field_editing {
                        *editing_field = None;
                        *master_field_editing = false;
                        *master_edit_field = None;
                        cancelled = true;
                    }
                }
                Ok(())
            })?;
            if cancelled {
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
                Ok(())
            } else {
                // No nested edit active: leave dashboard
                handle_leave_page(bus)?;
                Ok(())
            }
        }
        KeyCode::Enter => {
            // Edit entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Delete | KeyCode::Char('x') => {
            // Delete entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Char('n') => {
            // New entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Tab => {
            // Tab switching
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle leaving the modbus dashboard back to entry page
fn handle_leave_page(bus: &Bus) -> Result<()> {
    use crate::protocol::status::write_status;
    use crate::tui::utils::bus::UiToCore;

    write_status(|s| {
        // Go back to entry page
        s.page = types::Page::Entry { cursor: None };
        Ok(())
    })?;
    bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
    Ok(())
}
