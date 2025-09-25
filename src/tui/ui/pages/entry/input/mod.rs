pub mod navigation;
pub mod actions;

pub use navigation::{handle_input, handle_move_next, handle_move_prev};
pub use actions::handle_mouse;
