pub mod components;
pub mod input;
pub mod render;

pub use input::{handle_input, handle_mouse};
pub use render::{page_bottom_hints, render};
