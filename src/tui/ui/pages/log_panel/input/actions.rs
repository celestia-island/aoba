use anyhow::{anyhow, Result};

use crate::{
    protocol::status::{read_status, types, write_status},
    tui::utils::bus::{Bus, UiToCore},
};

pub fn handle_leave_page(bus: &Bus) -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::LogPanel { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    write_status(|status| {
        status.page = types::Page::ConfigPanel {
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
    // Toggle follow functionality - simplified for now
    log::info!("Toggle follow functionality");
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}

pub fn handle_clear_logs(bus: &Bus) -> Result<()> {
    // Clear logs functionality - simplified for now
    log::info!("Clear logs functionality");
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}