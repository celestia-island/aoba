pub mod actions;
pub mod navigation;

pub use actions::handle_mouse;
pub use navigation::{handle_input, handle_move_next, handle_move_prev};
