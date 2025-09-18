use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{self, Status},
        write_status,
    },
    tui::utils::bus::{Bus, UiToCore},
};

/// Handle input for log panel. Sends commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Snapshot previously provided by caller as `app`
    let snapshot = read_status(|s| Ok(s.clone()))?;
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
            // Navigation commands within the log
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Esc | KeyCode::Char('h') => {
            // Leave page - go back to entry
            handle_leave_page(bus)?;
            Ok(())
        }
        KeyCode::Char('f') => {
            // Toggle follow mode
            handle_toggle_follow(bus, &snapshot)?;
            Ok(())
        }
        KeyCode::Char('c') => {
            // Clear logs
            handle_clear_logs(bus, &snapshot)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle leaving the log panel back to entry page
fn handle_leave_page(bus: &Bus) -> Result<()> {
    let cursor = read_status(|s| {
        if let types::Page::ModbusLog { selected_port, .. } = &s.page {
            Ok(Some(types::ui::EntryCursor::Com { idx: *selected_port }))
        } else {
            Ok(None)
        }
    })?;
    write_status(|s| {
        // Go back to entry page
        s.page = types::Page::Entry { cursor };
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

/// Handle toggling follow mode for logs
fn handle_toggle_follow(bus: &Bus, app: &Status) -> Result<()> {
    // Toggle the auto-scroll flag for the current port
    if let types::Page::ModbusLog { selected_port, .. } = &app.page {
        write_status(|s| {
            if let Some(port_name) = s.ports.order.get(*selected_port) {
                if let Some(port_data) = s.ports.map.get_mut(port_name) {
                    port_data.log_auto_scroll = !port_data.log_auto_scroll;
                }
            }
            Ok(())
        })?;
    }
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

/// Handle clearing logs for the current port
fn handle_clear_logs(bus: &Bus, app: &Status) -> Result<()> {
    // Clear logs for the current port
    if let types::Page::ModbusLog { selected_port, .. } = &app.page {
        write_status(|s| {
            if let Some(port_name) = s.ports.order.get(*selected_port) {
                if let Some(port_data) = s.ports.map.get_mut(port_name) {
                    port_data.logs.clear();
                    port_data.log_selected = 0;
                }
            }
            Ok(())
        })?;
    }
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}
