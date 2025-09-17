use anyhow::{anyhow, Result};

use crossterm::event::KeyCode;

use crate::{
    protocol::status::types::{self, Status},
    tui::utils::bus::Bus,
};

/// Handle input for log panel. Sends commands via UiToCore.
pub fn handle_input(key: crossterm::event::KeyEvent, app: &Status, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
            // Navigation commands within the log
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Esc | KeyCode::Char('h') => {
            // Leave page - go back to entry
            handle_leave_page(bus);
            Ok(())
        }
        KeyCode::Char('f') => {
            // Toggle follow mode
            handle_toggle_follow(bus, app);
            Ok(())
        }
        KeyCode::Char('c') => {
            // Clear logs
            handle_clear_logs(bus, app);
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle leaving the log panel back to entry page
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

/// Handle toggling follow mode for logs
fn handle_toggle_follow(bus: &Bus, app: &Status) {
    use crate::protocol::status::write_status;
    use crate::tui::utils::bus::UiToCore;

    // Toggle the auto-scroll flag for the current port
    if let types::Page::ModbusLog { selected_port, .. } = &app.page {
        let _ = write_status(|s| {
            if let Some(port_name) = s.ports.order.get(*selected_port) {
                if let Some(port_data) = s.ports.map.get_mut(port_name) {
                    port_data.log_auto_scroll = !port_data.log_auto_scroll;
                }
            }
            Ok(())
        });
    }
    let _ = bus.ui_tx.send(UiToCore::Refresh);
}

/// Handle clearing logs for the current port
fn handle_clear_logs(bus: &Bus, app: &Status) {
    use crate::protocol::status::write_status;
    use crate::tui::utils::bus::UiToCore;

    // Clear logs for the current port
    if let types::Page::ModbusLog { selected_port, .. } = &app.page {
        let _ = write_status(|s| {
            if let Some(port_name) = s.ports.order.get(*selected_port) {
                if let Some(port_data) = s.ports.map.get_mut(port_name) {
                    port_data.logs.clear();
                    port_data.log_selected = 0;
                }
            }
            Ok(())
        });
    }
    let _ = bus.ui_tx.send(UiToCore::Refresh);
}
