/// Utilities for monitoring and parsing status dump files from TUI/CLI processes
///
/// This module provides functions to read and parse status dump JSON files
/// written by TUI and CLI processes when running in debug CI E2E test mode.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

/// Status dump structure for TUI processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiStatus {
    pub ports: Vec<DebugPort>,
    pub page: String,
    pub timestamp: String,
}

/// Status dump structure for CLI subprocess
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStatus {
    pub port_name: String,
    pub station_id: u8,
    pub register_mode: String,
    pub register_address: u16,
    pub register_length: u16,
    pub mode: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugPort {
    pub name: String,
    pub enabled: bool,
    pub state: String,
    pub modbus_masters: Vec<DebugModbusMaster>,
    pub modbus_slaves: Vec<DebugModbusSlave>,
    pub log_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugModbusMaster {
    pub station_id: u8,
    pub register_type: String,
    pub start_address: u16,
    pub register_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugModbusSlave {
    pub station_id: u8,
    pub register_type: String,
    pub start_address: u16,
    pub register_count: usize,
}

/// Read and parse TUI status from /tmp/ci_tui_status.json
pub fn read_tui_status() -> Result<TuiStatus> {
    let path = PathBuf::from("/tmp/ci_tui_status.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|err| anyhow!("Failed to read TUI status file {}: {}", path.display(), err))?;

    serde_json::from_str(&content)
        .map_err(|err| anyhow!("Failed to parse TUI status JSON: {}", err))
}

/// Read and parse CLI status from /tmp/ci_cli_{port}_status.json
/// Port is the base filename (e.g., "/tmp/vcom1" -> "vcom1")
pub fn read_cli_status(port: &str) -> Result<CliStatus> {
    // Extract basename from port path (e.g., "/tmp/vcom1" -> "vcom1")
    let port_basename = std::path::Path::new(port)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(port);

    let path = PathBuf::from(format!("/tmp/ci_cli_{}_status.json", port_basename));
    let content = std::fs::read_to_string(&path)
        .map_err(|err| anyhow!("Failed to read CLI status file {}: {}", path.display(), err))?;

    serde_json::from_str(&content)
        .map_err(|err| anyhow!("Failed to parse CLI status JSON: {}", err))
}

/// Wait for TUI to reach a specific page with timeout and retry logic
///
/// # Arguments
/// * `expected_page` - The page name to wait for (e.g., "Entry", "ModbusDashboard")
/// * `timeout_secs` - Total timeout in seconds
/// * `retry_interval_ms` - Interval between retries in milliseconds (default: 500ms)
///
/// # Returns
/// The TuiStatus when the expected page is reached, or an error if timeout is exceeded
pub async fn wait_for_tui_page(
    expected_page: &str,
    timeout_secs: u64,
    retry_interval_ms: Option<u64>,
) -> Result<TuiStatus> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let interval = retry_interval_ms.unwrap_or(500);

    loop {
        if start.elapsed() > timeout.into() {
            return Err(anyhow!(
                "Timeout waiting for TUI page '{}' (waited {}s)",
                expected_page,
                timeout_secs
            ));
        }

        if let Ok(status) = read_tui_status() {
            if status.page == expected_page {
                log::info!("✅ TUI reached page '{}'", expected_page);
                return Ok(status);
            }
            log::debug!(
                "TUI currently on page '{}', waiting for '{}'",
                status.page,
                expected_page
            );
        }

        sleep(Duration::from_millis(interval)).await;
    }
}

/// Wait for a port to be enabled in TUI with timeout and retry logic
///
/// # Arguments
/// * `port_name` - The port name to check (e.g., "/tmp/vcom1")
/// * `timeout_secs` - Total timeout in seconds
/// * `retry_interval_ms` - Interval between retries in milliseconds (default: 500ms)
///
/// # Returns
/// The TuiStatus when the port is enabled, or an error if timeout is exceeded
pub async fn wait_for_port_enabled(
    port_name: &str,
    timeout_secs: u64,
    retry_interval_ms: Option<u64>,
) -> Result<TuiStatus> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let interval = retry_interval_ms.unwrap_or(500);

    loop {
        if start.elapsed() > timeout.into() {
            return Err(anyhow!(
                "Timeout waiting for port '{}' to be enabled (waited {}s)",
                port_name,
                timeout_secs
            ));
        }

        if let Ok(status) = read_tui_status() {
            for port in &status.ports {
                if port.name == port_name && port.enabled {
                    log::info!("✅ Port '{}' is enabled", port_name);
                    return Ok(status);
                }
            }
            log::debug!("Port '{}' not yet enabled, retrying...", port_name);
        }

        sleep(Duration::from_millis(interval)).await;
    }
}

/// Wait for a port to have a specific modbus configuration (master or slave)
///
/// # Arguments
/// * `port_name` - The port name to check
/// * `is_master` - True to check for master configuration, false for slave
/// * `station_id` - Expected station ID
/// * `timeout_secs` - Total timeout in seconds
/// * `retry_interval_ms` - Interval between retries in milliseconds (default: 500ms)
///
/// # Returns
/// The TuiStatus when the configuration is detected, or an error if timeout is exceeded
pub async fn wait_for_modbus_config(
    port_name: &str,
    is_master: bool,
    station_id: u8,
    timeout_secs: u64,
    retry_interval_ms: Option<u64>,
) -> Result<TuiStatus> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let interval = retry_interval_ms.unwrap_or(500);
    let role = if is_master { "master" } else { "slave" };

    loop {
        if start.elapsed() > timeout.into() {
            return Err(anyhow!(
                "Timeout waiting for port '{}' to have {} station {} (waited {}s)",
                port_name,
                role,
                station_id,
                timeout_secs
            ));
        }

        if let Ok(status) = read_tui_status() {
            for port in &status.ports {
                if port.name == port_name {
                    let found = if is_master {
                        port.modbus_masters
                            .iter()
                            .any(|m| m.station_id == station_id)
                    } else {
                        port.modbus_slaves
                            .iter()
                            .any(|s| s.station_id == station_id)
                    };

                    if found {
                        log::info!(
                            "✅ Port '{}' has {} station {}",
                            port_name,
                            role,
                            station_id
                        );
                        return Ok(status);
                    }
                }
            }
            log::debug!(
                "Port '{}' does not have {} station {} yet, retrying...",
                port_name,
                role,
                station_id
            );
        }

        sleep(Duration::from_millis(interval)).await;
    }
}

/// Wait for CLI subprocess to start and write its status
///
/// # Arguments
/// * `port_name` - The port name the CLI subprocess is using
/// * `timeout_secs` - Total timeout in seconds
/// * `retry_interval_ms` - Interval between retries in milliseconds (default: 500ms)
///
/// # Returns
/// The CliStatus when it's available, or an error if timeout is exceeded
pub async fn wait_for_cli_status(
    port_name: &str,
    timeout_secs: u64,
    retry_interval_ms: Option<u64>,
) -> Result<CliStatus> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let interval = retry_interval_ms.unwrap_or(500);

    loop {
        if start.elapsed() > timeout.into() {
            return Err(anyhow!(
                "Timeout waiting for CLI status for port '{}' (waited {}s)",
                port_name,
                timeout_secs
            ));
        }

        if let Ok(status) = read_cli_status(port_name) {
            log::info!(
                "✅ CLI subprocess status available for port '{}'",
                port_name
            );
            return Ok(status);
        }

        sleep(Duration::from_millis(interval)).await;
    }
}

/// Helper to check if a port exists in the current TUI status
pub fn port_exists_in_tui(port_name: &str) -> Result<bool> {
    let status = read_tui_status()?;
    Ok(status.ports.iter().any(|p| p.name == port_name))
}

/// Helper to get the number of logs for a port
pub fn get_port_log_count(port_name: &str) -> Result<usize> {
    let status = read_tui_status()?;
    for port in &status.ports {
        if port.name == port_name {
            return Ok(port.log_count);
        }
    }
    Err(anyhow!("Port '{}' not found in TUI status", port_name))
}
