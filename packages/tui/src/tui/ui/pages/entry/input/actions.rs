use anyhow::Result;

use crossterm::event::MouseEventKind;

use super::navigation::{handle_move_next, handle_move_prev};
use crate::tui::status::types;
use crate::tui::utils::bus::Bus;

pub fn handle_mouse(event: crossterm::event::MouseEvent, _bus: &Bus) -> Result<()> {
    match event.kind {
        MouseEventKind::ScrollUp => {
            handle_move_prev(types::cursor::EntryCursor::Refresh)?;
        }
        MouseEventKind::ScrollDown => {
            handle_move_next(types::cursor::EntryCursor::Refresh)?;
        }
        _ => {}
    }
    Ok(())
}
