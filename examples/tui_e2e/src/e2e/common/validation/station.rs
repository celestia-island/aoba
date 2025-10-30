use serde_json::json;

use super::super::status_paths::station_field_path;
use ci_utils::CursorAction;

/// Validate that a station was created with specific configuration.
pub fn check_station_config(
    port_name: &str,
    station_index: usize,
    is_master: bool,
    station_id: u8,
    register_type: &str,
    start_address: u16,
    register_count: u16,
) -> Vec<CursorAction> {
    vec![
        CursorAction::CheckStatus {
            description: format!("Station {} ID is {}", station_index + 1, station_id),
            path: station_field_path(port_name, is_master, station_index, "station_id"),
            expected: json!(station_id),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
        CursorAction::CheckStatus {
            description: format!(
                "Station {} register type is {}",
                station_index + 1,
                register_type
            ),
            path: station_field_path(port_name, is_master, station_index, "register_type"),
            expected: json!(register_type),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
        CursorAction::CheckStatus {
            description: format!(
                "Station {} start address is 0x{:04X}",
                station_index + 1,
                start_address
            ),
            path: station_field_path(port_name, is_master, station_index, "start_address"),
            expected: json!(start_address),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
        CursorAction::CheckStatus {
            description: format!(
                "Station {} register count is {}",
                station_index + 1,
                register_count
            ),
            path: station_field_path(port_name, is_master, station_index, "register_count"),
            expected: json!(register_count as usize),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
    ]
}
