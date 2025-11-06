use anyhow::{anyhow, Result};

use crate::tui::status as types;
use crate::tui::status::{read_status, write_status};
use crate::tui::utils::bus::{Bus, UiToCore};

pub fn handle_leave_page(bus: &Bus) -> Result<()> {
    let selected_port = read_status(|status| {
        if let crate::tui::status::Page::LogPanel { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    write_status(|status| {
        status.page = crate::tui::status::Page::ConfigPanel {
            cursor: types::cursor::ConfigPanelCursor::ViewCommunicationLog,
            selected_port,
            view_offset: 0,
        };
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

pub fn handle_toggle_follow(bus: &Bus) -> Result<()> {
    // Toggle auto-follow by switching selected_item between None and Some
    write_status(|status| {
        if let crate::tui::status::Page::LogPanel {
            selected_item,
            selected_port,
            ..
        } = &mut status.page
        {
            match selected_item {
                None => {
                    // Currently in auto-follow mode, switch to manual mode
                    // Get the current log count and set to last item
                    if let Some(port_name) = status.ports.order.get(*selected_port) {
                        if let Some(port) = status.ports.map.get(port_name) {
                            let port_data = port;
                            let log_count = port_data.logs.len();
                            if log_count > 0 {
                                *selected_item = Some(log_count - 1);
                            }
                        }
                    }
                }
                Some(_) => {
                    // Currently in manual mode, switch to auto-follow mode
                    *selected_item = None;
                }
            }
        }
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

pub fn handle_clear_logs(bus: &Bus) -> Result<()> {
    // Clear logs functionality - actually clear the logs for the current port
    write_status(|status| {
        if let crate::tui::status::Page::LogPanel { selected_port, .. } = &status.page {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get_mut(port_name) {
                    port.logs.clear();
                    log::info!("Cleared logs for port: {port_name}");
                }
            }
        }
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}
