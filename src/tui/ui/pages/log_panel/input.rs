use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{read_status, types, write_status},
    tui::utils::bus::{Bus, UiToCore},
};

/// Handle input for log panel. Sends commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::PageUp => {
            // Scroll up
            crate::tui::ui::pages::log_panel::components::log_panel_scroll_up(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::PageDown => {
            // Scroll down
            crate::tui::ui::pages::log_panel::components::log_panel_scroll_down(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
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
            handle_toggle_follow(bus)?;
            Ok(())
        }
        KeyCode::Char('c') => {
            // Clear logs
            handle_clear_logs(bus)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle leaving the log panel back to config panel
fn handle_leave_page(bus: &Bus) -> Result<()> {
    let selected_port = read_status(|s| {
        if let types::Page::LogPanel { selected_port, .. } = &s.page {
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
            cursor: crate::protocol::status::types::cursor::ConfigPanelCursor::EnablePort,
        };
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

/// Handle toggling follow mode for logs
fn handle_toggle_follow(bus: &Bus) -> Result<()> {
    // Toggle the auto-scroll flag for the current port
    if let types::Page::LogPanel { selected_port, .. } = read_status(|s| Ok(s.page.clone()))? {
        write_status(|s| {
            if let Some(port_name) = s.ports.order.get(selected_port) {
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
fn handle_clear_logs(bus: &Bus) -> Result<()> {
    // Clear logs for the current port
    if let types::Page::LogPanel { selected_port, .. } = read_status(|s| Ok(s.page.clone()))? {
        write_status(|s| {
            if let Some(port_name) = s.ports.order.get(selected_port) {
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
