use anyhow::Result;

use crossterm::event::{KeyCode, KeyEvent, MouseEventKind};

use super::scroll::{handle_scroll_down, handle_scroll_up};
use crate::{
    protocol::status::{ types},
    tui::{
        ui::pages::entry::{calculate_special_items_offset, CONSERVATIVE_VIEWPORT_HEIGHT},
        utils::bus::Bus,
    },
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    const PAGE_SIZE: usize = 10;

    // Check for Ctrl modifier
    let has_ctrl = key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL);

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
            if has_ctrl {
                // Ctrl+PageUp: Jump to first scroll position
                write_status(|status| {
                    if let crate::tui::status::Page::About { view_offset, .. } = &mut status.page {
                        *view_offset = 0;
                    }
                    Ok(())
                })?;
            } else {
                // PageUp: Scroll up by page size
                handle_scroll_up(PAGE_SIZE)?;
            }
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::PageDown => {
            if has_ctrl {
                // Ctrl+PageDown: Jump to last scroll position
                // We don't know the exact content height here, so just scroll down a lot
                // The scroll handler will clamp to max position
                handle_scroll_down(1000)?;
            } else {
                // PageDown: Scroll down by page size
                handle_scroll_down(PAGE_SIZE)?;
            }
            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh)?;
            Ok(())
        }
        KeyCode::Esc => {
            let new_cursor = types::cursor::EntryCursor::About;
            let ports_count = read_status(|status| Ok(status.ports.order.len()))?;
            let offset = calculate_special_items_offset(ports_count, CONSERVATIVE_VIEWPORT_HEIGHT);
            write_status(|status| {
                status.page = crate::tui::status::Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: offset,
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
