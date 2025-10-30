use serde_json::json;

use super::status_paths::page_type_path;
use ci_utils::CursorAction;

mod configure;
mod connection;
mod creation;
mod focus;
mod persistence;
mod registers;

pub use configure::{
    configure_register_count, configure_register_type, configure_start_address,
    configure_station_id,
};
pub use connection::ensure_connection_mode;
pub use creation::create_station;
pub use focus::{focus_create_station_button, focus_station};
pub use persistence::save_configuration_and_verify;
pub use registers::initialize_slave_registers;

const MODBUS_DASHBOARD_PAGE: &str = "modbus_dashboard";

fn modbus_page_check(description: &str) -> CursorAction {
    CursorAction::CheckStatus {
        description: description.to_string(),
        path: page_type_path().to_string(),
        expected: json!(MODBUS_DASHBOARD_PAGE),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }
}
