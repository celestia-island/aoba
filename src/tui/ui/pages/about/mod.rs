pub mod input;
pub mod render;

pub use input::{handle_input, handle_mouse};
pub use render::{global_hints, page_bottom_hints, render};
