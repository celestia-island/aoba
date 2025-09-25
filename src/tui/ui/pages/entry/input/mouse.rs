use anyhow::Result;

use crossterm::event::MouseEventKind;

use crate::{
    protocol::status::types,
    tui::utils::bus::Bus,
};

use super::cursor_move::{handle_move_next, handle_move_prev};

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