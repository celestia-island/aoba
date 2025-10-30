use anyhow::Result;
use expectrl::Expect;
use serde_json::json;

use super::super::status_paths::port_field_path;
use super::modbus_page_check;
use ci_utils::{execute_with_status_checks, CursorAction, TerminalCapture};

/// Saves the configuration and verifies that the port is enabled.
///
/// This function presses Ctrl+S to save the configuration and then checks
/// the status file to ensure the port's `enabled` flag is set to `true`.
pub async fn save_configuration_and_verify<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
) -> Result<()> {
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressCtrlS, CursorAction::Sleep3s],
        &[
            CursorAction::CheckStatus {
                description: "Port is enabled".to_string(),
                path: port_field_path(port_name, "enabled"),
                expected: json!(true),
                timeout_secs: Some(10),
                retry_interval_ms: Some(500),
            },
            modbus_page_check("ModbusDashboard active after saving configuration"),
        ],
        "save_configuration",
        Some(3),
    )
    .await
}
