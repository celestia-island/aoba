use crate::protocol::status::types::{self, Status};
use crate::tui::utils::bus::Bus;
use anyhow::{anyhow, Result};

/// Handle input for ModBus panel. Sends commands via UiToCore.
pub fn handle_input(key: crossterm::event::KeyEvent, _app: &Status, bus: &Bus) -> Result<()> {
    use crossterm::event::KeyCode as KC;

    match key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Navigation within the dashboard
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KC::Left | KC::Right => {
            // Horizontal navigation within fields
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KC::Esc => {
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
            });
            if cancelled {
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
                Ok(())
            } else {
                // No nested edit active: leave dashboard
                handle_leave_page(bus);
                Ok(())
            }
        }
        KC::Enter => {
            // Edit entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KC::Delete | KC::Char('x') => {
            // Delete entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KC::Char('n') => {
            // New entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KC::Tab => {
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
fn handle_leave_page(bus: &Bus) {
    use crate::protocol::status::write_status;
    use crate::tui::utils::bus::UiToCore;

    let _ = write_status(|s| {
        // Go back to entry page
        s.page = types::Page::Entry { cursor: None };
        Ok(())
    });
    let _ = bus.ui_tx.send(UiToCore::Refresh);
}
