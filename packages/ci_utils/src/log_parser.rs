/// Utilities for parsing and verifying TUI log files in E2E tests.
///
/// This module provides functions to extract STATE_DUMP entries from log files
/// and verify state transitions without relying on terminal screen capture.
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::Path;

/// A parsed state dump from the TUI log file
#[derive(Debug, Clone)]
pub struct StateDump {
    pub page: String,
    pub cursor: String,
    pub ports: Vec<PortState>,
    pub config_edit: ConfigEditState,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PortState {
    pub name: String,
    pub state: String,
    pub port_type: String,
}

#[derive(Debug, Clone)]
pub struct ConfigEditState {
    pub active: bool,
    pub port: Option<String>,
    pub field_index: usize,
    pub field_key: Option<String>,
    pub buffer: String,
    pub cursor_pos: usize,
}

/// Read and parse all STATE_DUMP entries from a log file
pub fn parse_state_dumps<P: AsRef<Path>>(log_path: P) -> Result<Vec<StateDump>> {
    let content = std::fs::read_to_string(log_path.as_ref())
        .map_err(|err| anyhow!("Failed to read log file: {err}"))?;

    let mut dumps = Vec::new();

    for line in content.lines() {
        if let Some(json_start) = line.find("STATE_DUMP: ") {
            let json_str = &line[json_start + "STATE_DUMP: ".len()..];
            match serde_json::from_str::<Value>(json_str) {
                Ok(json) => {
                    let dump = parse_dump_json(&json)?;
                    dumps.push(dump);
                }
                Err(err) => {
                    log::warn!("Failed to parse STATE_DUMP JSON: {err}");
                    continue;
                }
            }
        }
    }

    Ok(dumps)
}

/// Parse a single STATE_DUMP JSON object into a StateDump struct
fn parse_dump_json(json: &Value) -> Result<StateDump> {
    let page = json
        .get("page")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let cursor = json
        .get("cursor")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A")
        .to_string();

    let mut ports = Vec::new();
    if let Some(ports_arr) = json.get("ports").and_then(|v| v.as_array()) {
        for port_json in ports_arr {
            let name = port_json
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let state = port_json
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let port_type = port_json
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ports.push(PortState {
                name,
                state,
                port_type,
            });
        }
    }

    let config_edit = if let Some(ce) = json.get("config_edit") {
        ConfigEditState {
            active: ce.get("active").and_then(|v| v.as_bool()).unwrap_or(false),
            port: ce
                .get("port")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            field_index: ce.get("field_index").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            field_key: ce
                .get("field_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            buffer: ce
                .get("buffer")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            cursor_pos: ce.get("cursor_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        }
    } else {
        ConfigEditState {
            active: false,
            port: None,
            field_index: 0,
            field_key: None,
            buffer: String::new(),
            cursor_pos: 0,
        }
    };

    let error = json
        .get("error")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(StateDump {
        page,
        cursor,
        ports,
        config_edit,
        error,
    })
}

/// Find the most recent state dump in the log file
pub fn get_latest_state<P: AsRef<Path>>(log_path: P) -> Result<StateDump> {
    let dumps = parse_state_dumps(log_path)?;
    dumps
        .into_iter()
        .last()
        .ok_or_else(|| anyhow!("No STATE_DUMP entries found in log"))
}

/// Wait for a specific page to appear in the log (with timeout)
pub async fn wait_for_page<P: AsRef<Path>>(
    log_path: P,
    expected_page: &str,
    timeout_secs: u64,
) -> Result<StateDump> {
    use tokio::time::{sleep, Duration};

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for page '{expected_page}' (waited {timeout_secs}s)"
            ));
        }

        if let Ok(state) = get_latest_state(log_path.as_ref()) {
            if state.page == expected_page {
                return Ok(state);
            }
        }

        sleep(Duration::from_millis(100)).await;
    }
}

/// Wait for a specific port state to appear in the log (with timeout)
pub async fn wait_for_port_state<P: AsRef<Path>>(
    log_path: P,
    port_name: &str,
    expected_state: &str,
    timeout_secs: u64,
) -> Result<StateDump> {
    use tokio::time::{sleep, Duration};

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for port '{port_name}' to reach state '{expected_state}' (waited {timeout_secs}s)"
            ));
        }

        if let Ok(state) = get_latest_state(log_path.as_ref()) {
            for port in &state.ports {
                if port.name == port_name && port.state == expected_state {
                    return Ok(state);
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }
}

/// Verify that a port exists in the latest state dump
pub fn verify_port_exists<P: AsRef<Path>>(log_path: P, port_name: &str) -> Result<()> {
    let state = get_latest_state(log_path)?;
    for port in &state.ports {
        if port.name == port_name {
            return Ok(());
        }
    }
    Err(anyhow!("Port '{port_name}' not found in state dump"))
}

/// Get the state of a specific port from the latest state dump
pub fn get_port_state<P: AsRef<Path>>(log_path: P, port_name: &str) -> Result<String> {
    let state = get_latest_state(log_path)?;
    for port in &state.ports {
        if port.name == port_name {
            return Ok(port.state.clone());
        }
    }
    Err(anyhow!("Port '{port_name}' not found in latest state dump"))
}
