use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{self, cursor::Cursor},
        write_status,
    },
    tui::utils::bus::{Bus, UiToCore},
};

use super::actions::{handle_enter_action, handle_leave_page};

pub fn handle_navigation_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;
            let new_cursor = current_cursor.prev();
            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;
            let new_cursor = current_cursor.next();
            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Enter => {
            handle_enter_action(bus)?;
            Ok(())
        }
        KeyCode::Delete => {
            // Handle delete key functionality here
            Ok(())
        }
        KeyCode::Esc => {
            handle_leave_page(bus)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
