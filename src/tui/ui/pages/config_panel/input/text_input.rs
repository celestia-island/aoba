use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{
            self,
        },
        write_status,
    },
    tui::utils::bus::Bus,
};

use super::{cursor_move::sanitize_configpanel_cursor, scroll::{handle_scroll_down, handle_scroll_up}};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    sanitize_configpanel_cursor()?;

    let editing = read_status(|status| {
        Ok(!matches!(
            status.temporarily.input_raw_buffer,
            types::ui::InputRawBuffer::None
        ))
    })?;

    if editing {
        // Handle editing mode
        handle_editing_input(key, bus)?;
    } else {
        // Handle navigation mode
        handle_navigation_input(key, bus)?;
    }
    Ok(())
}

fn handle_editing_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Enter => {
            // Commit the current edit
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Esc => {
            // Cancel the current edit
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        _ => {
            // Handle text input
            Ok(())
        }
    }
}

fn handle_navigation_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::PageUp => {
            handle_scroll_up(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::PageDown => {
            handle_scroll_down(5)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Handle cursor movement up
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Handle cursor movement down
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Enter => {
            // Handle selection/edit entry
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        KeyCode::Esc => {
            // Return to entry page
            write_status(|status| {
                if let types::Page::ConfigPanel { selected_port, .. } = &status.page {
                    status.page = types::Page::Entry {
                        cursor: Some(types::cursor::EntryCursor::Com {
                            index: *selected_port,
                        }),
                    };
                } else {
                    status.page = types::Page::Entry { cursor: None };
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        _ => Ok(()),
    }
}