use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use super::actions::{handle_clear_logs, handle_leave_page, handle_toggle_follow};
use crate::tui::{status::write_status, utils::bus::Bus};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Conservative estimate of viewport height (total height minus title, borders, bottom hints)
    // Similar to CONSERVATIVE_VIEWPORT_HEIGHT in entry page
    const VIEWPORT_HEIGHT: usize = 20;

    // Check for Ctrl modifier
    let has_ctrl = key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL);

    match key.code {
        KeyCode::PageUp => {
            if has_ctrl {
                // Ctrl+PageUp: Jump to first log item
                write_status(|status| {
                    if let crate::tui::status::Page::LogPanel { selected_item, .. } =
                        &mut status.page
                    {
                        *selected_item = Some(0);
                    }
                    Ok(())
                })?;
            } else {
                // PageUp: Scroll up by one page (viewport height)
                crate::tui::ui::pages::log_panel::components::handle_scroll_up(VIEWPORT_HEIGHT)?;
            }
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::PageDown => {
            if has_ctrl {
                // Ctrl+PageDown: Jump to last log item
                write_status(|status| {
                    if let crate::tui::status::Page::LogPanel {
                        selected_item,
                        selected_port,
                        ..
                    } = &mut status.page
                    {
                        if let Some(port_name) = status.ports.order.get(*selected_port) {
                            if let Some(port) = status.ports.map.get(port_name) {
                                let port_data = port;
                                let log_count = port_data.logs.len();
                                if log_count > 0 {
                                    *selected_item = Some(log_count.saturating_sub(1));
                                }
                            }
                        }
                    }
                    Ok(())
                })?;
            } else {
                // PageDown: Scroll down by one page (viewport height)
                crate::tui::ui::pages::log_panel::components::handle_scroll_down(VIEWPORT_HEIGHT)?;
            }
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Up => {
            crate::tui::ui::pages::log_panel::components::handle_scroll_up(1)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Down => {
            crate::tui::ui::pages::log_panel::components::handle_scroll_down(1)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Enter => {
            // Handle Enter key for input area editing
            write_status(|status| {
                // Set input mode to editing or toggle between modes
                if let crate::tui::status::Page::LogPanel { input_mode, .. } = &mut status.page {
                    *input_mode = match input_mode {
                        crate::tui::status::ui::InputMode::Ascii => {
                            crate::tui::status::ui::InputMode::Hex
                        }
                        crate::tui::status::ui::InputMode::Hex => {
                            crate::tui::status::ui::InputMode::Ascii
                        }
                    };
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Char('k') => {
            crate::tui::ui::pages::log_panel::components::handle_scroll_up(1)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Char('j') => {
            crate::tui::ui::pages::log_panel::components::handle_scroll_down(1)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Esc | KeyCode::Char('h') => {
            handle_leave_page(bus)?;
            Ok(())
        }
        KeyCode::Char('v') => {
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
