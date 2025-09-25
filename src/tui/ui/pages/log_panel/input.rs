use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{read_status, types, with_port_write, write_status},
    tui::utils::bus::{Bus, UiToCore},
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::PageUp => {
            crate::tui::ui::pages::log_panel::components::log_panel_scroll_up(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::PageDown => {
            crate::tui::ui::pages::log_panel::components::log_panel_scroll_down(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Esc | KeyCode::Char('h') => {
            handle_leave_page(bus)?;
            Ok(())
        }
        KeyCode::Char('f') => {
            handle_toggle_follow(bus)?;
            Ok(())
        }
        KeyCode::Char('c') => {
            handle_clear_logs(bus)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_leave_page(bus: &Bus) -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::LogPanel { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    write_status(|status| {
        status.page = types::Page::ConfigPanel {
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

fn handle_toggle_follow(bus: &Bus) -> Result<()> {
    if let types::Page::LogPanel { selected_port, .. } =
        read_status(|status| Ok(status.page.clone()))?
    {
        write_status(|status| {
            if let Some(port_name) = status.ports.order.get(selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if with_port_write(port, |port| {
                        port.log_auto_scroll = !port.log_auto_scroll;
                    })
                    .is_some()
                    {
                    } else {
                        log::warn!(
                            "handle_toggle_follow: failed to acquire write lock for {port_name}"
                        );
                    }
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

fn handle_clear_logs(bus: &Bus) -> Result<()> {
    if let types::Page::LogPanel { selected_port, .. } =
        read_status(|status| Ok(status.page.clone()))?
    {
        write_status(|status| {
            if let Some(port_name) = status.ports.order.get(selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if with_port_write(port, |port| {
                        port.logs.clear();
                    })
                    .is_some()
                    {
                    } else {
                        log::warn!(
                            "handle_clear_logs: failed to acquire write lock for {port_name}"
                        );
                    }
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
