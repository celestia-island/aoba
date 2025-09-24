use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{read_status, types, write_status},
    tui::utils::bus::Bus,
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => Ok(()),
        KeyCode::Left | KeyCode::Right => Ok(()),
        KeyCode::Esc => {
            handle_leave_page(bus)?;
            Ok(())
        }
        KeyCode::Enter => Ok(()),
        _ => Ok(()),
    }
}

fn handle_leave_page(bus: &Bus) -> Result<()> {
    use crate::tui::utils::bus::UiToCore;

    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    write_status(|status| {
        // Go back to config panel instead of entry page
        status.page = types::Page::ConfigPanel {
            selected_port,
            view_offset: 0,
            cursor: types::cursor::ConfigPanelCursor::EnablePort,
        };
        Ok(())
    })?;
    bus.ui_tx
        .send(UiToCore::Refresh)
        .map_err(|err| anyhow!(err))?;
    Ok(())
}
