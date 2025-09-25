use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::utils::bus::Bus;

use super::actions::{handle_clear_logs, handle_leave_page, handle_toggle_follow};

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
