//! Placeholder value management
//!
//! Handles {{#N}} placeholders for storing and retrieving test data values.

use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use anyhow::Result;

/// Global placeholder storage
static PLACEHOLDERS: Lazy<Mutex<HashMap<usize, String>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

/// Store a value in a placeholder slot
pub fn set_placeholder(index: usize, value: String) {
    let mut placeholders = PLACEHOLDERS.lock().unwrap();
    placeholders.insert(index, value);
    log::debug!("📌 Placeholder #{} = {}", index, placeholders.get(&index).unwrap());
}

/// Get a value from a placeholder slot
pub fn get_placeholder(index: usize) -> Result<String> {
    let placeholders = PLACEHOLDERS.lock().unwrap();
    placeholders.get(&index)
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
        },
        "hex" => {
            // Generate random hex value 0x0000-0xFFFF
            format!("{:04X}", rng.random_range(0..0x10000))
        },
        "binary" => {
            // Generate random binary value
            format!("{:08b}", rng.random_range(0..256))
        },
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
}
