pub mod cursor_move;
pub mod mouse;
pub mod navigation;

pub use cursor_move::{handle_move_next, handle_move_prev};
pub use mouse::handle_mouse;
pub use navigation::handle_input;
