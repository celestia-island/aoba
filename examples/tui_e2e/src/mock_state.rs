//! Mock state management for screen-capture testing mode
//!
//! This module manages a mock TUI global state that can be manipulated
//! and verified during screen-capture-only testing.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::{collections::HashMap, convert::TryFrom, sync::Mutex, time::Instant};

use aoba::tui::status::types::{
    cursor::{ConfigPanelCursor, ModbusDashboardCursor},
    modbus::{ModbusConnectionMode, ModbusRegisterItem, RegisterMode},
    port::{PortConfig, PortData, PortState, PortStatusIndicator},
    ui::InputMode,
};
use aoba::tui::status::{self, Page as TuiPage};

/// Global mock state storage
static MOCK_STATE: Lazy<Mutex<Value>> = Lazy::new(|| {
    Mutex::new(json!({
        "ports": {},
        "port_order": [],
        "page": "entry",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
});

fn default_port(name: &str) -> Value {
    json!({
        "name": name,
        "enabled": false,
        "state": "Free",
        "modbus_masters": [],
        "modbus_slaves": [],
        "log_count": 0
    })
}

/// Initialize mock state with default structure
pub fn init_mock_state() {
    let mut state = MOCK_STATE.lock().unwrap();
    let mut ports = Map::new();
    ports.insert("/tmp/vcom1".to_string(), default_port("/tmp/vcom1"));
    ports.insert("/tmp/vcom2".to_string(), default_port("/tmp/vcom2"));

    let mut root = Map::new();
    root.insert("ports".to_string(), Value::Object(ports));
    root.insert(
        "port_order".to_string(),
        json!(["/tmp/vcom1", "/tmp/vcom2"]),
    );
    root.insert("page".to_string(), json!("entry"));
    root.insert(
        "timestamp".to_string(),
        json!(chrono::Utc::now().to_rfc3339()),
    );

    *state = Value::Object(root);
    log::debug!("🔧 Initialized mock state with default ports");
}

#[derive(Debug, Clone, Deserialize)]
struct MockStateData {
    ports: HashMap<String, MockPort>,
    #[serde(default)]
    port_order: Vec<String>,
    #[serde(default = "default_page_name")]
    page: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MockPort {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_port_state_name")]
    state: String,
    #[serde(default)]
    modbus_masters: Vec<MockModbusStation>,
    #[serde(default)]
    modbus_slaves: Vec<MockModbusStation>,
}

#[derive(Debug, Clone, Deserialize)]
struct MockModbusStation {
    #[serde(default)]
    station_id: u8,
    #[serde(default)]
    register_type: Option<String>,
    #[serde(default)]
    start_address: u16,
    #[serde(default)]
    register_count: usize,
    #[serde(default)]
    registers: Vec<Value>,
}

fn default_page_name() -> String {
    "entry".to_string()
}

fn default_port_state_name() -> String {
    "Free".to_string()
}

/// Synchronize the JSON mock state into the live TUI status tree so that
/// screen-capture rendering reflects the expected UI content.
pub fn sync_mock_state_to_tui_status() -> Result<()> {
    let snapshot = {
        let state = MOCK_STATE.lock().unwrap();
        state.clone()
    };

    let parsed: MockStateData = serde_json::from_value(snapshot)
        .context("Failed to deserialize mock state for TUI synchronization")?;

    let mut port_order = if parsed.port_order.is_empty() {
        let mut keys: Vec<String> = parsed.ports.keys().cloned().collect();
        keys.sort();
        keys
    } else {
        parsed.port_order.clone()
    };

    // Ensure the order only contains ports present in the map
    port_order.retain(|name| parsed.ports.contains_key(name));

    status::write_status(|status| {
        status.ports.order = port_order.clone();
        status.ports.map.clear();

        for name in &status.ports.order {
            if let Some(port) = parsed.ports.get(name) {
                let data = convert_port(name, port)?;
                status.ports.map.insert(name.clone(), data);
            }
        }

        status.page = convert_page(&parsed.page);

        Ok(())
    })?;

    Ok(())
}

fn convert_port(name: &str, port: &MockPort) -> Result<PortData> {
    let mut data = PortData::default();
    data.port_name = port
        .name
        .clone()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| name.to_string());

    data.state = parse_port_state(&port.state)?;

    if port.enabled && !matches!(data.state, PortState::OccupiedByThis) {
        data.state = PortState::OccupiedByThis;
    }

    data.status_indicator = match data.state {
        PortState::OccupiedByThis => PortStatusIndicator::Running,
        _ => PortStatusIndicator::NotStarted,
    };

    let (mode, stations_source) =
        if !port.modbus_slaves.is_empty() && port.modbus_masters.is_empty() {
            (ModbusConnectionMode::default_slave(), &port.modbus_slaves)
        } else {
            (ModbusConnectionMode::default_master(), &port.modbus_masters)
        };

    let mut stations = Vec::new();
    for station in stations_source {
        stations.push(convert_station(station)?);
    }

    data.config = PortConfig::Modbus { mode, stations };

    Ok(data)
}

fn parse_port_state(state: &str) -> Result<PortState> {
    match state {
        "Free" => Ok(PortState::Free),
        "OccupiedByThis" => Ok(PortState::OccupiedByThis),
        "OccupiedByOther" => Ok(PortState::OccupiedByOther),
        other => {
            log::warn!("Unknown port state '{}', defaulting to Free", other);
            Ok(PortState::Free)
        }
    }
}

fn convert_station(station: &MockModbusStation) -> Result<ModbusRegisterItem> {
    let register_mode = parse_register_mode(station.register_type.as_deref())?;
    let register_length =
        u16::try_from(station.register_count).context("register_count exceeds u16 range")?;

    let last_values = convert_register_values(station, register_length)?;

    Ok(ModbusRegisterItem {
        station_id: station.station_id,
        register_mode,
        register_address: station.start_address,
        register_length,
        last_values,
        req_success: 0,
        req_total: 0,
        next_poll_at: Instant::now(),
        last_request_time: None,
        last_response_time: None,
        pending_requests: Vec::new(),
    })
}

fn parse_register_mode(value: Option<&str>) -> Result<RegisterMode> {
    match value {
        Some(name) => RegisterMode::try_from(name)
            .or_else(|_| RegisterMode::try_from(name.to_ascii_lowercase().as_str()))
            .or_else(|_| RegisterMode::try_from(name.to_ascii_uppercase().as_str()))
            .or_else(|_| match name {
                "coils" => Ok(RegisterMode::Coils),
                "discrete_inputs" => Ok(RegisterMode::DiscreteInputs),
                "holding" => Ok(RegisterMode::Holding),
                "input" => Ok(RegisterMode::Input),
                _ => Err(()),
            })
            .map_err(|_| anyhow::anyhow!("Unsupported register type: {}", name)),
        None => Ok(RegisterMode::Coils),
    }
}

fn convert_register_values(station: &MockModbusStation, register_length: u16) -> Result<Vec<u16>> {
    let mut values = vec![0u16; register_length as usize];

    for (idx, raw) in station.registers.iter().enumerate() {
        if idx >= values.len() {
            break;
        }
        values[idx] = parse_u16_value(raw)
            .with_context(|| format!("Failed to parse register value at index {}", idx))?;
    }

    Ok(values)
}

fn parse_u16_value(value: &Value) -> Result<u16> {
    match value {
        Value::Number(num) => num
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(|| anyhow::anyhow!("Number out of u16 range")),
        Value::String(s) => {
            if let Some(stripped) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                u16::from_str_radix(stripped, 16)
                    .map_err(|_| anyhow::anyhow!("Invalid hex value: {}", s))
            } else {
                u16::from_str_radix(s, 10)
                    .map_err(|_| anyhow::anyhow!("Invalid decimal value: {}", s))
            }
        }
        Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
        Value::Null => Ok(0),
        other => Err(anyhow::anyhow!("Unsupported value type: {:?}", other)),
    }
}

fn convert_page(page: &str) -> TuiPage {
    match page.to_ascii_lowercase().as_str() {
        "config_panel" | "configpanel" => TuiPage::ConfigPanel {
            selected_port: 0,
            view_offset: 0,
            cursor: ConfigPanelCursor::EnablePort,
        },
        "modbus_dashboard" | "modbusdashboard" => TuiPage::ModbusDashboard {
            selected_port: 0,
            view_offset: 0,
            cursor: ModbusDashboardCursor::AddLine,
        },
        "log_panel" | "logpanel" => TuiPage::LogPanel {
            selected_port: 0,
            input_mode: InputMode::Ascii,
            selected_item: None,
        },
        "about" => TuiPage::About { view_offset: 0 },
        _ => TuiPage::Entry {
            cursor: None,
            view_offset: 0,
        },
    }
}

/// Set a value in mock state using path notation
/// Examples:
/// - "ports['/tmp/vcom1'].enabled" = true
/// - "ports['/tmp/vcom1'].modbus_masters[0].registers[5]" = 42
pub fn set_mock_state(path: &str, value: Value) -> Result<()> {
    let mut state = MOCK_STATE.lock().unwrap();

    // Parse the path and navigate to the target
    set_value_at_path(&mut state, path, value)
        .context(format!("Failed to set mock state at path: {}", path))?;

    log::debug!(
        "🔧 Mock state updated: {} = {:?}",
        path,
        get_value_at_path(&state, path).ok()
    );
    Ok(())
}

/// Get a value from mock state using path notation
pub fn get_mock_state(path: &str) -> Result<Value> {
    let state = MOCK_STATE.lock().unwrap();
    get_value_at_path(&state, path).context(format!("Failed to get mock state at path: {}", path))
}

/// Verify a value in mock state matches expected
pub fn verify_mock_state(path: &str, expected: &Value) -> Result<()> {
    let actual = get_mock_state(path)?;

    if &actual != expected {
        anyhow::bail!(
            "Mock state verification failed at path '{}'\n  Expected: {:?}\n  Actual: {:?}",
            path,
            expected,
            actual
        );
    }

    log::debug!("✅ Mock state verified: {} = {:?}", path, expected);
    Ok(())
}

/// Get the entire mock state for debugging
pub fn get_full_mock_state() -> Value {
    let state = MOCK_STATE.lock().unwrap();
    state.clone()
}

/// Save mock state to file (for debugging)
pub fn save_mock_state_to_file(path: &str) -> Result<()> {
    let state = get_full_mock_state();
    std::fs::write(path, serde_json::to_string_pretty(&state)?)?;
    log::debug!("💾 Saved mock state to: {}", path);
    Ok(())
}

// Helper functions for path navigation

fn set_value_at_path(root: &mut Value, path: &str, value: Value) -> Result<Value> {
    let parts = parse_path(path)?;
    let mut current = root;

    // Navigate to the parent of the target
    for (i, part) in parts.iter().enumerate() {
        let next_part = parts.get(i + 1);
        if i == parts.len() - 1 {
            // Last part - set the value
            match part {
                PathPart::Key(key) => {
                    if let Value::Object(map) = current {
                        map.insert(key.clone(), value.clone());
                    } else {
                        anyhow::bail!("Cannot set key '{}' on non-object", key);
                    }
                }
                PathPart::Index(idx) => {
                    if let Value::Array(arr) = current {
                        if *idx >= arr.len() {
                            // Extend array if needed
                            arr.resize(*idx + 1, Value::Null);
                        }
                        arr[*idx] = value.clone();
                    } else {
                        anyhow::bail!("Cannot set index {} on non-array", idx);
                    }
                }
            }
            return Ok(value);
        }

        // Navigate deeper
        current = match part {
            PathPart::Key(key) => {
                if let Value::Object(map) = current {
                    map.entry(key.clone())
                        .or_insert_with(|| default_value_for_next(next_part))
                } else {
                    anyhow::bail!("Cannot navigate key '{}' on non-object", key);
                }
            }
            PathPart::Index(idx) => {
                if let Value::Array(arr) = current {
                    if *idx >= arr.len() {
                        arr.resize_with(*idx + 1, || default_value_for_next(next_part));
                    }
                    if arr[*idx].is_null() {
                        arr[*idx] = default_value_for_next(next_part);
                    }
                    &mut arr[*idx]
                } else {
                    anyhow::bail!("Cannot navigate index {} on non-array", idx);
                }
            }
        };
    }

    Ok(value)
}

fn get_value_at_path(root: &Value, path: &str) -> Result<Value> {
    let parts = parse_path(path)?;
    let mut current = root;

    for part in &parts {
        current = match part {
            PathPart::Key(key) => {
                if let Value::Object(map) = current {
                    map.get(key)
                        .ok_or_else(|| anyhow::anyhow!("Key '{}' not found", key))?
                } else {
                    anyhow::bail!("Cannot access key '{}' on non-object", key);
                }
            }
            PathPart::Index(idx) => {
                if let Value::Array(arr) = current {
                    arr.get(*idx)
                        .ok_or_else(|| anyhow::anyhow!("Index {} out of bounds", idx))?
                } else {
                    anyhow::bail!("Cannot access index {} on non-array", idx);
                }
            }
        };
    }

    Ok(current.clone())
}

fn default_value_for_next(next_part: Option<&PathPart>) -> Value {
    match next_part {
        Some(PathPart::Index(_)) => Value::Array(Vec::new()),
        _ => Value::Object(Map::new()),
    }
}

#[derive(Debug)]
enum PathPart {
    Key(String),
    Index(usize),
}

fn parse_path(path: &str) -> Result<Vec<PathPart>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_brackets = false;
    let mut bracket_content = String::new();

    for ch in path.chars() {
        match ch {
            '[' => {
                if !current.is_empty() {
                    parts.push(PathPart::Key(current.clone()));
                    current.clear();
                }
                in_brackets = true;
                bracket_content.clear();
            }
            ']' => {
                if in_brackets {
                    // Try to parse as index first
                    if let Ok(idx) = bracket_content.parse::<usize>() {
                        parts.push(PathPart::Index(idx));
                    } else {
                        // Otherwise treat as string key (remove quotes if present)
                        let key = bracket_content.trim_matches(|c| c == '\'' || c == '"');
                        parts.push(PathPart::Key(key.to_string()));
                    }
                    in_brackets = false;
                    bracket_content.clear();
                }
            }
            '.' => {
                if !in_brackets && !current.is_empty() {
                    parts.push(PathPart::Key(current.clone()));
                    current.clear();
                } else if in_brackets {
                    bracket_content.push(ch);
                }
            }
            _ => {
                if in_brackets {
                    bracket_content.push(ch);
                } else {
                    current.push(ch);
                }
            }
        }
    }

    if !current.is_empty() {
        parts.push(PathPart::Key(current));
    }

    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_path() {
        let parts = parse_path("ports['/tmp/vcom1'].enabled").unwrap();
        assert_eq!(parts.len(), 2);

        let parts = parse_path("ports['/tmp/vcom1'].modbus_masters[0].registers[5]").unwrap();
        assert_eq!(parts.len(), 5);
    }

    #[test]
    fn test_mock_state_operations() {
        init_mock_state();

        // Set nested value
        set_mock_state("ports['/tmp/vcom1'].enabled", json!(true)).unwrap();

        // Get value back
        let value = get_mock_state("ports['/tmp/vcom1'].enabled").unwrap();
        assert_eq!(value, json!(true));

        // Verify value
        assert!(verify_mock_state("ports['/tmp/vcom1'].enabled", &json!(true)).is_ok());
        assert!(verify_mock_state("ports['/tmp/vcom1'].enabled", &json!(false)).is_err());
    }
}
