pub mod actions;
pub mod editing;
pub mod navigation;

use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::{
    protocol::status::{read_status, types},
    tui::utils::bus::Bus,
};

use editing::handle_editing_input;
use navigation::handle_navigation_input;

pub use actions::{handle_enter_action, handle_leave_page};
pub use editing::handle_editing_input as handle_edit_input;
pub use navigation::handle_navigation_input as handle_nav_input;

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    let editing = read_status(|status| {
        Ok(!matches!(
            status.temporarily.input_raw_buffer,
            types::ui::InputRawBuffer::None
        ))
    })?;

    if editing {
        handle_editing_input(key, bus)?;
    } else {
        handle_navigation_input(key, bus)?;
    }
    Ok(())
}
