use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{read_status, types, write_status},
    tui::utils::bus::Bus,
};

/// Handle input for ModBus panel. Sends commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::PageUp => {
            // Scroll up
            crate::tui::ui::pages::modbus_panel::components::modbus_panel_scroll_up(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::PageDown => {
            // Scroll down
            crate::tui::ui::pages::modbus_panel::components::modbus_panel_scroll_down(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
            // Navigation within the dashboard
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Left | KeyCode::Right => {
            // Horizontal navigation within fields
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Esc => {
            // If dashboard has nested edit state in Status (e.g. editing_field or master_field_editing),
            // prefer to cancel those first. Otherwise leave to entry page.
            let mut cancelled = false;
            write_status(|s| {
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
                    .map_err(|err| anyhow!(err))?;
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
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Delete | KeyCode::Char('x') => {
            // Delete entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Char('n') => {
            // New entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Tab => {
            // Tab switching
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle leaving the modbus dashboard back to entry page
/// Handle leaving the ModBus dashboard back to config panel
fn handle_leave_page(bus: &Bus) -> Result<()> {
    use crate::tui::utils::bus::UiToCore;

    let selected_port = read_status(|s| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &s.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    write_status(|s| {
        // Go back to config panel instead of entry page
        s.page = types::Page::ConfigPanel {
            selected_port,
            view_offset: 0,
            cursor: crate::protocol::status::types::ui::ConfigPanelCursor::EnablePort,
        };
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}
