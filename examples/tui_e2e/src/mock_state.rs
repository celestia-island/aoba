//! Mock state management for screen-capture testing mode
//!
//! This module manages a mock TUI global state that can be manipulated
//! and verified during screen-capture-only testing.

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use serde_json::{json, Map, Value};
use serde_json_path::JsonPath;
use std::sync::Mutex;

use aoba::tui::status::{self, TuiStatus};

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

/// Synchronize the JSON mock state into the live TUI status tree so that
/// screen-capture rendering reflects the expected UI content.
pub fn sync_mock_state_to_tui_status() -> Result<()> {
    let snapshot = {
        let state = MOCK_STATE.lock().unwrap();
        state.clone()
    };

    apply_mock_state_to_tui_status(&snapshot)
}

fn apply_mock_state_to_tui_status(snapshot: &Value) -> Result<()> {
    let mock: TuiStatus = serde_json::from_value(snapshot.clone())
        .context("Mock state must be convertible to TuiStatus")?;

    status::write_status(|status| mock.apply_to_status(status))?;

    Ok(())
}

/// Set a value in mock state using JSONPath-style notation.
/// Examples:
/// - "ports['/tmp/vcom1'].enabled" = true
/// - "ports['/tmp/vcom1'].modbus_masters[0].registers[5]" = 42
pub fn set_mock_state(path: &str, value: Value) -> Result<()> {
    let json_path = normalize_json_path(path);
    let mut state = MOCK_STATE.lock().unwrap();

    ensure_path_exists(&mut state, &json_path)
        .with_context(|| format!("Failed to ensure path '{}' exists", path))?;

    apply_value_at_path(&mut state, &json_path, value.clone())
        .with_context(|| format!("Failed to apply value at path '{}'", path))?;

    log::debug!("🔧 Mock state updated: {} = {:?}", path, value);
    Ok(())
}

/// Get a value from mock state using JSONPath-style notation.
pub fn get_mock_state(path: &str) -> Result<Value> {
    let json_path = normalize_json_path(path);
    let state = MOCK_STATE.lock().unwrap();

    let parsed = JsonPath::parse(&json_path)
        .with_context(|| format!("Invalid JSONPath expression: {}", json_path))?;

    let node = parsed
        .query(&*state)
        .exactly_one()
        .map_err(|err| anyhow!("Path '{}' must resolve to exactly one node: {err}", path))?;

    Ok(node.clone())
}

/// Verify a value in mock state matches expected value.
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

/// Get the entire mock state for debugging.
pub fn get_full_mock_state() -> Value {
    let state = MOCK_STATE.lock().unwrap();
    state.clone()
}

/// Save mock state to file (for debugging).
pub fn save_mock_state_to_file(path: &str) -> Result<()> {
    let state = get_full_mock_state();
    std::fs::write(path, serde_json::to_string_pretty(&state)?)?;
    log::debug!("💾 Saved mock state to: {}", path);
    Ok(())
}

fn apply_value_at_path(root: &mut Value, json_path: &str, value: Value) -> Result<()> {
    let pointer = {
        let parsed = JsonPath::parse(json_path)?;
        let located = parsed
            .query_located(&*root)
            .exactly_one()
            .map_err(|err| anyhow!("Path '{}' must match exactly one node: {err}", json_path))?;
        located.location().to_json_pointer()
    };

    if let Some(target) = root.pointer_mut(&pointer) {
        *target = value;
        Ok(())
    } else {
        Err(anyhow!(
            "Unable to resolve pointer '{}' derived from path '{}'",
            pointer,
            json_path
        ))
    }
}

fn ensure_path_exists(root: &mut Value, json_path: &str) -> Result<()> {
    let parts = parse_path(json_path)?;
    if parts.is_empty() {
        return Ok(());
    }

    let mut current = root;

    for (index, part) in parts.iter().enumerate() {
        let next_part = parts.get(index + 1);
        let is_last = index == parts.len() - 1;

        match part {
            PathPart::Key(key) => {
                if !current.is_object() {
                    *current = Value::Object(Map::new());
                }

                let map = current.as_object_mut().expect("Value ensured as object");

                if !map.contains_key(key) {
                    let default = if is_last {
                        Value::Null
                    } else {
                        default_value_for_next(next_part)
                    };
                    map.insert(key.clone(), default);
                } else if !is_last {
                    let entry = map.get_mut(key).expect("Entry exists after contains check");
                    if entry.is_null() {
                        *entry = default_value_for_next(next_part);
                    }
                }

                current = map.get_mut(key).expect("Entry exists after insertion");
            }
            PathPart::Index(idx) => {
                if !current.is_array() {
                    *current = Value::Array(Vec::new());
                }

                let array = current.as_array_mut().expect("Value ensured as array");

                let fill_value = default_value_for_next(next_part);
                if array.len() <= *idx {
                    array.resize_with(*idx + 1, || fill_value.clone());
                }

                if array[*idx].is_null() && !is_last {
                    array[*idx] = default_value_for_next(next_part);
                }

                current = array
                    .get_mut(*idx)
                    .expect("Array index populated during ensure path");
            }
        }
    }

    Ok(())
}

fn normalize_json_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "$".to_string();
    }

    if trimmed.starts_with('$') {
        trimmed.to_string()
    } else if trimmed.starts_with('[') {
        format!("${}", trimmed)
    } else {
        format!("$.{}", trimmed)
    }
}

fn default_value_for_next(next_part: Option<&PathPart>) -> Value {
    match next_part {
        Some(PathPart::Index(_)) => Value::Array(Vec::new()),
        Some(PathPart::Key(_)) => Value::Object(Map::new()),
        None => Value::Null,
    }
}

#[derive(Debug)]
enum PathPart {
    Key(String),
    Index(usize),
}

fn parse_path(path: &str) -> Result<Vec<PathPart>> {
    let mut trimmed = path.trim();
    if let Some(stripped) = trimmed.strip_prefix('$') {
        trimmed = stripped;
    }
    if let Some(stripped) = trimmed.strip_prefix('.') {
        trimmed = stripped;
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut bracket_content = String::new();
    let mut in_brackets = false;
    let mut in_quotes = false;
    let mut quote_char = '\0';

    for ch in trimmed.chars() {
        match ch {
            '[' if !in_brackets => {
                if !current.is_empty() {
                    parts.push(PathPart::Key(current.clone()));
                    current.clear();
                }
                in_brackets = true;
                bracket_content.clear();
            }
            ']' if in_brackets && !in_quotes => {
                if bracket_content.is_empty() {
                    anyhow::bail!("Empty bracket expression in path {}", path);
                }

                if let Ok(index) = bracket_content.parse::<usize>() {
                    parts.push(PathPart::Index(index));
                } else {
                    let key = bracket_content.trim_matches(|c| c == '\'' || c == '"');
                    parts.push(PathPart::Key(key.to_string()));
                }

                in_brackets = false;
                bracket_content.clear();
            }
            '.' if !in_brackets => {
                if !current.is_empty() {
                    parts.push(PathPart::Key(current.clone()));
                    current.clear();
                }
            }
            '\'' | '"' if in_brackets => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if quote_char == ch {
                    in_quotes = false;
                }
                bracket_content.push(ch);
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
        assert_eq!(parts.len(), 3);

        let parts = parse_path("ports['/tmp/vcom1'].modbus_masters[0].registers[5]").unwrap();
        assert_eq!(parts.len(), 6);

        let parts = parse_path("$.ports['/tmp/vcom1'].enabled").unwrap();
        assert_eq!(parts.len(), 3);
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
