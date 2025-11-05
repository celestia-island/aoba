//! Placeholder value management
//!
//! Handles {{#N}} placeholders for storing and retrieving test data values.

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::collections::HashMap;
use std::sync::Mutex;

/// Global placeholder storage
static PLACEHOLDERS: Lazy<Mutex<HashMap<usize, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Register pool storage separated by data type so workflows can reuse
/// deterministic random values across multiple steps.
#[derive(Default)]
struct RegisterPools {
    bools: Vec<bool>,
    ints: Vec<u16>,
}

static REGISTER_POOLS: Lazy<Mutex<RegisterPools>> =
    Lazy::new(|| Mutex::new(RegisterPools::default()));

/// Available register pool categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterPoolKind {
    Bool,
    Int,
}

/// Unified representation returned from the register pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterValue {
    Bool(bool),
    Int(u16),
}

impl RegisterValue {
    /// Return the value as JSON using natural Modbus semantics (bool/number).
    pub fn to_json(self) -> Value {
        match self {
            RegisterValue::Bool(b) => Number::from(if b { 1 } else { 0 }).into(),
            RegisterValue::Int(v) => Number::from(v as u64).into(),
        }
    }
}

/// Initialize the register pools with randomized contents sized by workflow manifest metadata.
pub fn init_register_pools(bool_count: usize, int_count: usize) {
    let mut pools = REGISTER_POOLS.lock().unwrap();

    pools.bools = (0..bool_count).map(|index| index % 2 == 0).collect();
    pools.ints = (0..int_count)
        .map(|index| ((index as u32 * 0x1357 + 0x24) & 0xFFFF) as u16)
        .collect();

    log::debug!(
        "🎲 Initialized register pools (bools={}, ints={})",
        pools.bools.len(),
        pools.ints.len()
    );
}

/// Fetch a register value from the requested pool.
pub fn get_register_value(kind: RegisterPoolKind, index: usize) -> Result<RegisterValue> {
    let pools = REGISTER_POOLS.lock().unwrap();

    match kind {
        RegisterPoolKind::Bool => pools
            .bools
            .get(index)
            .copied()
            .map(RegisterValue::Bool)
            .ok_or_else(|| anyhow!("Boolean register index {} out of range", index)),
        RegisterPoolKind::Int => pools
            .ints
            .get(index)
            .copied()
            .map(RegisterValue::Int)
            .ok_or_else(|| anyhow!("Integer register index {} out of range", index)),
    }
}

/// Return whether the specified register evaluates to a truthy value.
pub fn register_truthy(kind: RegisterPoolKind, index: usize) -> Result<bool> {
    match get_register_value(kind, index)? {
        RegisterValue::Bool(value) => Ok(value),
        RegisterValue::Int(value) => Ok(value != 0),
    }
}

/// Format a register value as a string for typing or placeholder substitution.
pub fn register_value_as_string(value: RegisterValue, format: Option<&str>) -> String {
    match value {
        RegisterValue::Bool(b) => match format.unwrap_or("bool") {
            "bool" => b.to_string(),
            "one_zero" => if b { "1" } else { "0" }.to_string(),
            "yes_no" => if b { "yes" } else { "no" }.to_string(),
            other => {
                log::warn!(
                    "Unknown bool format '{}', defaulting to 'true/false'",
                    other
                );
                b.to_string()
            }
        },
        RegisterValue::Int(v) => match format.unwrap_or("decimal") {
            "decimal" => v.to_string(),
            "hex" => format!("{:04X}", v),
            "hex_lower" => format!("{:04x}", v),
            "binary" => format!("{:016b}", v),
            other => {
                log::warn!("Unknown integer format '{}', defaulting to decimal", other);
                v.to_string()
            }
        },
    }
}

/// Convert a register value into JSON using the natural representation for the pool.
pub fn register_value_to_json(value: RegisterValue) -> Value {
    value.to_json()
}

/// Store a value in a placeholder slot
pub fn set_placeholder(index: usize, value: String) {
    let mut placeholders = PLACEHOLDERS.lock().unwrap();
    placeholders.insert(index, value);
    log::debug!(
        "📌 Placeholder #{} = {}",
        index,
        placeholders.get(&index).unwrap()
    );
}

/// Get a value from a placeholder slot
pub fn get_placeholder(index: usize) -> Result<String> {
    let placeholders = PLACEHOLDERS.lock().unwrap();
    placeholders
        .get(&index)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Placeholder #{} not found", index))
}

/// Replace all placeholders in a string with their values
/// Supports:
/// - {{#N}} - decimal value
/// - {{0x#N}} - hexadecimal value
/// - {{0b#N}} - binary value
pub fn replace_placeholders(text: &str) -> Result<String> {
    let mut result = text.to_string();

    // Replace register references first so stored placeholder replacements can reuse them later.
    let re_bool_reg = regex::Regex::new(r"\{\{bool\[(\d+)\]\}\}").unwrap();
    for cap in re_bool_reg.captures_iter(&result.clone()) {
        let index: usize = cap[1].parse()?;
        let value = get_register_value(RegisterPoolKind::Bool, index)?;
        let text_value = register_value_as_string(value, Some("bool"));
        result = result.replace(&cap[0], &text_value);
    }

    let re_int_reg = regex::Regex::new(r"\{\{(0x|0b)?int\[(\d+)\]\}\}").unwrap();
    for cap in re_int_reg.captures_iter(&result.clone()) {
        let format_prefix = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let index: usize = cap[2].parse()?;
        let value = get_register_value(RegisterPoolKind::Int, index)?;

        let formatted = match format_prefix {
            "0x" => register_value_as_string(value, Some("hex")),
            "0b" => register_value_as_string(value, Some("binary")),
            _ => register_value_as_string(value, Some("decimal")),
        };

        result = result.replace(&cap[0], &formatted);
    }

    // Find all placeholder patterns
    let re_hex = regex::Regex::new(r"\{\{0x#(\d+)\}\}").unwrap();
    let re_bin = regex::Regex::new(r"\{\{0b#(\d+)\}\}").unwrap();
    let re_dec = regex::Regex::new(r"\{\{#(\d+)\}\}").unwrap();

    // Replace hex placeholders
    for cap in re_hex.captures_iter(&result.clone()) {
        let index: usize = cap[1].parse()?;
        let value = get_placeholder(index)?;
        let hex_value = if let Ok(num) = value.parse::<u64>() {
            format!("0x{:04X}", num)
        } else {
            value
        };
        result = result.replace(&cap[0], &hex_value);
    }

    // Replace binary placeholders
    for cap in re_bin.captures_iter(&result.clone()) {
        let index: usize = cap[1].parse()?;
        let value = get_placeholder(index)?;
        let bin_value = if let Ok(num) = value.parse::<u64>() {
            format!("0b{:08b}", num)
        } else {
            value
        };
        result = result.replace(&cap[0], &bin_value);
    }

    // Replace decimal placeholders
    for cap in re_dec.captures_iter(&result.clone()) {
        let index: usize = cap[1].parse()?;
        let value = get_placeholder(index)?;
        result = result.replace(&cap[0], &value);
    }

    Ok(result)
}

/// Clear all placeholders
pub fn clear_placeholders() {
    let mut placeholders = PLACEHOLDERS.lock().unwrap();
    placeholders.clear();
    log::debug!("🧹 Cleared all placeholders");
}

/// Generate a random test value based on input type
pub fn generate_value(input_type: &str, _format: Option<&str>) -> String {
    use rand::Rng;
    let mut rng = rand::rng();

    match input_type {
        "decimal" => {
            // Generate random decimal value 0-9999
            rng.random_range(0..10000).to_string()
        }
        "hex" => {
            // Generate random hex value 0x0000-0xFFFF
            format!("{:04X}", rng.random_range(0..0x10000))
        }
        "binary" => {
            // Generate random binary value
            format!("{:08b}", rng.random_range(0..256))
        }
        _ => {
            log::warn!("Unknown input type '{}', generating decimal", input_type);
            rng.random_range(0..10000).to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_storage() {
        clear_placeholders();
        set_placeholder(0, "1234".to_string());
        set_placeholder(5, "5678".to_string());

        assert_eq!(get_placeholder(0).unwrap(), "1234");
        assert_eq!(get_placeholder(5).unwrap(), "5678");
        assert!(get_placeholder(10).is_err());
    }

    #[test]
    fn test_replace_placeholders() {
        clear_placeholders();
        set_placeholder(0, "42".to_string());
        set_placeholder(1, "255".to_string());

        let result = replace_placeholders("Value is {{#0}} or {{0x#1}}").unwrap();
        assert_eq!(result, "Value is 42 or 0x00FF");
    }

    #[test]
    fn test_generate_value() {
        let dec = generate_value("decimal", None);
        assert!(dec.parse::<u32>().is_ok());

        let hex = generate_value("hex", None);
        assert_eq!(hex.len(), 4);
        assert!(u32::from_str_radix(&hex, 16).is_ok());
    }

    #[test]
    fn test_register_pools_and_placeholders() {
        init_register_pools(2, 2);

        // Ensure deterministic placeholder replacement does not panic
        let replaced = replace_placeholders("Bool={{bool[1]}}, Int={{int[0]}}, Hex={{0xint[1]}}")
            .expect("placeholder replacement should succeed");
        assert!(replaced.contains("Bool="));
        assert!(replaced.contains("Int="));
        assert!(replaced.contains("Hex="));
    }
}
