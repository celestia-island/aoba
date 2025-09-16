pub mod input;
pub mod render;

pub use input::handle_input;
pub use render::{global_hints, page_bottom_hints, render};
