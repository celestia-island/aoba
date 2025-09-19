use anyhow::Result;

use crossterm::event::{KeyCode, KeyEvent, MouseEventKind};

use crate::{
    protocol::status::{types, write_status},
    tui::{
        ui::pages::about::components::{about_scroll_down, about_scroll_up},
        utils::bus::Bus,
    },
};

/// Handle input for about page. Sends navigation commands via UiToCore.
/// Handle input for about page. Sends navigation commands via UiToCore.
pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    const PAGE_SIZE: usize = 10;

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            about_scroll_up(1)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            about_scroll_down(1)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::PageUp => {
            about_scroll_up(PAGE_SIZE)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::PageDown => {
            about_scroll_down(PAGE_SIZE)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::Esc => {
            write_status(|s| {
                s.page = types::Page::Entry {
                    cursor: Some(types::cursor::EntryCursor::About),
                };
                Ok(())
            })?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle mouse events for About page (scroll wheel). Return true when consumed.
pub fn handle_mouse(me: crossterm::event::MouseEvent, _bus: &Bus) -> Result<()> {
    match me.kind {
        MouseEventKind::ScrollUp => {
            about_scroll_up(1)?;
            Ok(())
        }
        MouseEventKind::ScrollDown => {
            about_scroll_down(1)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
