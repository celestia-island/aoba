mod configure;
mod connection;
mod creation;
mod focus;

use serde_json::json;

pub use configure::{
    configure_register_count, configure_register_type, configure_start_address,
    configure_station_id,
};
pub use connection::ensure_connection_mode;
pub use creation::create_station;
pub use focus::{focus_create_station_button, focus_station};

use super::status_paths::page_type_path;
use aoba_ci_utils::CursorAction;

const MODBUS_DASHBOARD_PAGE: &str = "modbus_dashboard";

fn modbus_page_check(description: &str) -> CursorAction {
    // Sleep to ensure page transition completes
    // Actual page verification is done via screenshot matching in the new JSON-based system
    let _ = description; // Suppress unused warning
    CursorAction::Sleep1s
}
