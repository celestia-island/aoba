pub mod display;
pub mod table;

pub use display::{derive_selection, render_kv_lines_with_indicators, render_modbus_status_lines};
pub use table::render_register_row_line;

