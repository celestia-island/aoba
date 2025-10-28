/// Common validation patterns for TUI E2E tests
///
/// This module provides reusable validation patterns that combine cursor actions
/// with status checks for fine-grained validation.

use anyhow::Result;
use ci_utils::*;
use expectrl::Expect;
use serde_json::{json, Value};

/// Validate that a station was created with specific configuration
///
/// # Arguments
/// * `port_index` - Index of the port in ports array (usually 0)
/// * `station_index` - Index of the station within the port's masters/slaves (0-based)
/// * `is_master` - Whether checking a master (true) or slave (false) station
/// * `station_id` - Expected station ID
/// * `register_type` - Expected register type (e.g., "Coil", "Holding")
/// * `start_address` - Expected start address
/// * `register_count` - Expected number of registers
pub fn check_station_config(
    port_index: usize,
    station_index: usize,
    is_master: bool,
    station_id: u8,
    register_type: &str,
    start_address: u16,
    register_count: u16,
) -> Vec<CursorAction> {
    let station_type = if is_master {
        "modbus_masters"
    } else {
        "modbus_slaves"
    };

    vec![
        CursorAction::CheckStatus {
            description: format!("Station {} ID is {}", station_index + 1, station_id),
            path: format!(
                "ports[{}].{}[{}].station_id",
                port_index, station_type, station_index
            ),
            expected: json!(station_id),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
        CursorAction::CheckStatus {
            description: format!("Station {} register type is {}", station_index + 1, register_type),
            path: format!(
                "ports[{}].{}[{}].register_type",
                port_index, station_type, station_index
            ),
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
            path: format!(
                "ports[{}].{}[{}].start_address",
                port_index, station_type, station_index
            ),
            expected: json!(start_address),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
        CursorAction::CheckStatus {
            description: format!("Station {} register count is {}", station_index + 1, register_count),
            path: format!(
                "ports[{}].{}[{}].register_count",
                port_index, station_type, station_index
            ),
            expected: json!(register_count as usize),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
    ]
}

/// Validate that we're on a specific page
pub fn check_page(page_name: &str) -> Vec<CursorAction> {
    vec![CursorAction::CheckStatus {
        description: format!("On {} page", page_name),
        path: "page".to_string(),
        expected: json!({"type": page_name}),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }]
}

/// Validate that a port is enabled
pub fn check_port_enabled(port_index: usize, enabled: bool) -> Vec<CursorAction> {
    vec![CursorAction::CheckStatus {
        description: format!(
            "Port {} is {}",
            port_index,
            if enabled { "enabled" } else { "disabled" }
        ),
        path: format!("ports[{}].enabled", port_index),
        expected: json!(enabled),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }]
}

/// Edit a field with fine-grained validation
///
/// This helper demonstrates the pattern of checking after each step:
/// 1. Navigate to field
/// 2. Enter edit mode
/// 3. Clear and type value
/// 4. Commit and verify
///
/// # Example
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use ci_utils::*;
///
/// // Edit station ID field to value 5
/// edit_field_with_validation(
///     &mut session,
///     &mut cap,
///     &[CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 }],
///     "5",
///     "ports[0].modbus_masters[0].station_id",
///     5,
///     "station_id",
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn edit_field_with_validation<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    navigate_actions: &[CursorAction],
    value_to_type: &str,
    status_path: &str,
    expected_value: Value,
    field_name: &str,
) -> Result<()> {
    // Step 1: Navigate to field
    execute_with_status_checks(
        session,
        cap,
        navigate_actions,
        &[],  // No status check yet - just navigation
        &format!("navigate_to_{}", field_name),
        None,
    )
    .await?;

    // Step 2: Enter edit mode
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter],
        &[],  // Could add edit mode check if cursor tracking exists
        &format!("enter_edit_{}", field_name),
        None,
    )
    .await?;

    // Step 3: Clear existing value and type new value
    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString(value_to_type.to_string()),
        ],
        &[],  // Could add buffer content check if cursor tracking exists
        &format!("type_{}", field_name),
        None,
    )
    .await?;

    // Step 4: Commit and verify value was written to config
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter],
        &[CursorAction::CheckStatus {
            description: format!("{} updated to {}", field_name, value_to_type),
            path: status_path.to_string(),
            expected: expected_value,
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        &format!("commit_{}", field_name),
        Some(3),  // 3 retries
    )
    .await?;

    Ok(())
}
