pub mod display;
pub mod scroll;

pub use display::{extract_log_data, render_log_display, render_log_input};
pub use scroll::{handle_scroll_down, handle_scroll_up};

