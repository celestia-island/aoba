use anyhow::{anyhow, Result};

use expectrl::Expect;

use super::super::status_paths::{station_collection, wait_for_station_count};
use aoba_ci_utils::{
    execute_cursor_actions, read_tui_status, CursorAction, ExpectSession, TerminalCapture,
};

/// Creates a new station and verifies its creation via a status check.
///
/// This function encapsulates the actions and verification for creating a single station.
/// It presses "Enter" on the "Create Station" button and then uses a `CheckStatus`
/// action to confirm that the station appears in the status file.
///
/// # Arguments
///
/// * `session` - The expectrl session to interact with the TUI.
/// * `cap` - The terminal capture utility for debugging.
/// * `port_name` - The name of the port where the station is being created.
/// * `is_master` - A boolean indicating whether the created station should be a master or a slave.
///
/// # Returns
///
/// * `Result<usize>` - Index of the newly created station in the status tree.
pub async fn create_station<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    is_master: bool,
) -> Result<usize> {
    let status = read_tui_status()?;
    let port = status
        .ports
        .iter()
        .find(|p| p.name == port_name)
        .ok_or_else(|| anyhow!("Port {port_name} not found when creating station"))?;

    let current_count = if is_master {
        port.modbus_masters.len()
    } else {
        port.modbus_slaves.len()
    };

    log::info!(
        "📊 Current {} station count before create: {}",
        if is_master { "master" } else { "slave" },
        current_count
    );

    let new_index = current_count;
    let collection = station_collection(is_master);

    execute_cursor_actions(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep3s],
        "create_station",
    )
    .await?;

    execute_cursor_actions(
        session,
        cap,
        &[CursorAction::Sleep1s],
        "create_station_page_check",
    )
    .await?;

    wait_for_station_count(port_name, is_master, new_index + 1, 10)
        .await
        .map_err(|err| {
            anyhow!(
                "Station list on {port_name} did not reach {} entries within timeout: {err}",
                new_index + 1
            )
        })?;

    log::info!(
        "✅ Station created at index {} ({} collection)",
        new_index,
        collection
    );

    Ok(new_index)
}
