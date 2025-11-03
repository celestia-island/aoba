mod configure;
mod connection;
mod creation;
mod focus;

pub use configure::{
    configure_register_count, configure_register_type, configure_start_address,
    configure_station_id,
};
pub use connection::ensure_connection_mode;
pub use creation::create_station;
pub use focus::{focus_create_station_button, focus_station};

use aoba_ci_utils::CursorAction;

fn modbus_page_check(_description: &str) -> CursorAction {
    // Sleep to ensure page transition completes
    // Actual page verification is done via screenshot matching in the new JSON-based system
    // `_description` is intentionally unused; leading underscore suppresses the warning.
    CursorAction::Sleep1s
}
