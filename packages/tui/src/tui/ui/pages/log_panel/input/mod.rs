pub mod actions;
pub mod navigation;

pub use actions::{handle_clear_logs, handle_leave_page, handle_toggle_follow};
pub use navigation::handle_input;

