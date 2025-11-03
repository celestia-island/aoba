use anyhow::{anyhow, Result};
use serde_json::to_string;
use tokio::time::{sleep, Duration};

use aoba_ci_utils::read_tui_status;

/// Build a JSONPath segment that filters ports by name.
pub(super) fn port_selector(port_name: &str) -> String {
    // serde_json::to_string already adds surrounding quotes and escapes special characters
    let literal = to_string(port_name).expect("Port name serialization should not fail");
    format!("ports[?(@.name == {literal})]")
}

/// Build a JSONPath for a field under the selected port.
pub(super) fn port_field_path(port_name: &str, field: &str) -> String {
    let selector = port_selector(port_name);
    if field.is_empty() {
        selector
    } else {
        format!("{selector}.{field}")
    }
}

/// Return the JSON key for master vs slave station collections.
pub(super) fn station_collection(is_master: bool) -> &'static str {
    if is_master {
        "modbus_masters"
    } else {
        "modbus_slaves"
    }
}

/// Build a JSONPath to a specific station entry under a port.
pub(super) fn station_path(port_name: &str, is_master: bool, station_index: usize) -> String {
    let collection = station_collection(is_master);
    let base = port_field_path(port_name, collection);
    format!("{base}[{station_index}]")
}

/// Build a JSONPath to a specific field of a station entry.
pub(super) fn station_field_path(
    port_name: &str,
    is_master: bool,
    station_index: usize,
    field: &str,
) -> String {
    let station = station_path(port_name, is_master, station_index);
    if field.is_empty() {
        station
    } else {
        format!("{station}.{field}")
    }
}

// `page_type_path` and `PAGE_TYPE_PATH` were removed because they're not used in the
// current E2E status-based verification flow.

/// Wait until the specified port reports at least `expected_count` stations in the TUI status dump.
pub async fn wait_for_station_count(
    port_name: &str,
    is_master: bool,
    expected_count: usize,
    timeout_secs: u64,
) -> Result<()> {
    let started = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(500);

    loop {
        if started.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for port {port_name} to report {expected_count} {} station(s)",
                if is_master { "master" } else { "slave" }
            ));
        }

        match read_tui_status() {
            Ok(status) => {
                if let Some(port) = status.ports.iter().find(|p| p.name == port_name) {
                    let current = if is_master {
                        port.modbus_masters.len()
                    } else {
                        port.modbus_slaves.len()
                    };

                    if current >= expected_count {
                        return Ok(());
                    }
                }
            }
            Err(err) => {
                log::debug!("wait_for_station_count: failed to read TUI status: {err}");
            }
        }

        sleep(interval).await;
    }
}
