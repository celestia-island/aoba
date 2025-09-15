pub mod render;
pub mod input;

pub use render::{global_hints, page_bottom_hints, render};
pub use input::{handle_input, handle_mouse};
