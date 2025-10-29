/// Common validation patterns for TUI E2E tests
///
/// This module provides reusable validation patterns that combine cursor actions
/// with status checks for fine-grained validation.
use super::status_paths::{page_type_path, station_field_path};
use ci_utils::*;
use serde_json::json;

/// Validate that a station was created with specific configuration
///
/// # Arguments
/// * `port_name` - Name of the port in the status tree (e.g., "/tmp/vcom1")
/// * `station_index` - Index of the station within the port's masters/slaves (0-based)
/// * `is_master` - Whether checking a master (true) or slave (false) station
/// * `station_id` - Expected station ID
/// * `register_type` - Expected register type (e.g., "Coil", "Holding")
/// * `start_address` - Expected start address
/// * `register_count` - Expected number of registers
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

/// Validate that the UI reports the expected page type.
pub fn check_page(expected_page: &str) -> Vec<CursorAction> {
    vec![CursorAction::CheckStatus {
        description: format!("Page is {expected_page}"),
        path: page_type_path().to_string(),
        expected: json!(expected_page),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }]
}
