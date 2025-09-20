use anyhow::Result;

use crossterm::event::{KeyCode, KeyEvent, MouseEventKind};

use crate::{
    protocol::status::{types, write_status},
    tui::utils::bus::Bus,
};

/// Scroll the About page view offset up by `amount` (saturating at 0).
pub fn handle_scroll_up(amount: usize) -> Result<()> {
    write_status(|s| {
        if let types::Page::About { view_offset } = &mut s.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the About page view offset down by `amount`.
pub fn handle_scroll_down(amount: usize) -> Result<()> {
    write_status(|s| {
        if let types::Page::About { view_offset } = &mut s.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}

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
