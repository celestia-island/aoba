//! Mock state management for screen-capture testing mode
//!
//! This module manages a mock TUI global state that can be manipulated
//! and verified during screen-capture-only testing.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::sync::Mutex;

/// Global mock state storage
static MOCK_STATE: Lazy<Mutex<Value>> = Lazy::new(|| {
    Mutex::new(json!({
        "ports": {},
        "page": "Entry",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
});

/// Initialize mock state with default structure
pub fn init_mock_state() {
    let mut state = MOCK_STATE.lock().unwrap();
    *state = json!({
        "ports": {},
        "page": "Entry",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    log::debug!("🔧 Initialized mock state");
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
                    map.entry(key.clone()).or_insert(json!({}))
                } else {
                    anyhow::bail!("Cannot navigate key '{}' on non-object", key);
                }
            }
            PathPart::Index(idx) => {
                if let Value::Array(arr) = current {
                    if *idx >= arr.len() {
                        arr.resize(*idx + 1, json!({}));
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
