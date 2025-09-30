use anyhow::Result;

use crossterm::event::{KeyCode, KeyEvent, MouseEventKind};

use super::scroll::{handle_scroll_down, handle_scroll_up};
use crate::{
    protocol::status::{types, types::cursor::Cursor, write_status},
    tui::utils::bus::Bus,
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    const PAGE_SIZE: usize = 10;

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            handle_scroll_up(1)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            handle_scroll_down(1)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::PageUp => {
            handle_scroll_up(PAGE_SIZE)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::PageDown => {
            handle_scroll_down(PAGE_SIZE)?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::Esc => {
            let new_cursor = types::cursor::EntryCursor::About;
            write_status(|status| {
                status.page = types::Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: new_cursor.view_offset(),
                };
                Ok(())
            })?;
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn handle_mouse(event: crossterm::event::MouseEvent, _bus: &Bus) -> Result<()> {
    match event.kind {
        MouseEventKind::ScrollUp => {
            handle_scroll_up(1)?;
            Ok(())
        }
        MouseEventKind::ScrollDown => {
            handle_scroll_down(1)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
