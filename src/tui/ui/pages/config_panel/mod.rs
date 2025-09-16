// Module root for `pages::config_panel` placed in `config_panel/mod.rs`.
// This mirrors the earlier shim in `config_panel.rs` so the module can live
// inside the `config_panel/` directory.

pub mod input;
pub mod render;

// Re-export commonly used symbols so external call sites that referenced
// `pages::config_panel::render` and `pages::config_panel::handle_input`
// remain valid without changes.
pub use input::handle_input;
pub use render::{global_hints, page_bottom_hints, render, render_kv_panel};
