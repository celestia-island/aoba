use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::Path;

/// Verify continuous data collected by CLI slave (shared helper)
pub fn verify_continuous_data(
    output_file: &Path,
    expected_values_list: &[Vec<u16>],
    _is_coil: bool,
) -> Result<()> {
    while !output_file.exists() {
        return Err(anyhow!("Output file does not exist"));
    }

    let content = std::fs::read_to_string(output_file)?;
    if content.trim().is_empty() {
        return Err(anyhow!("Output file is empty"));
    }

    let mut parsed_outputs = Vec::new();
    for (i, line) in content.lines().enumerate() {
        match serde_json::from_str::<Value>(line) {
            Ok(json) => {
                if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                    let values_u16: Vec<u16> = values
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u16))
                        .collect();
                    parsed_outputs.push(values_u16);
                }
            }
            Err(e) => {
                log::warn!(
                    "Line {line} is not valid JSON: {err}",
                    line = i + 1,
                    err = e
                );
            }
        }
    }

    let mut found_count = 0;
    for (i, expected_values) in expected_values_list.iter().enumerate() {
        let found = parsed_outputs
            .iter()
            .any(|output| output == expected_values);
        if found {
            log::info!(
                "Expected value set {idx} found: {vals:?}",
                idx = i + 1,
                vals = expected_values
            );
            found_count += 1;
        } else {
            log::warn!(
                "Expected value set {idx} NOT found: {vals:?}",
                idx = i + 1,
                vals = expected_values
            );
        }
    }

    if found_count == 0 {
        return Err(anyhow!(
            "None of the expected value sets were found in output"
        ));
    }

    Ok(())
}

/// Verify CLI output contains expected register values (used by master test)
pub fn verify_cli_output(output: &str) -> Result<()> {
    let expected_values = vec![0u16, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110];
    let mut all_found = true;
    for &val in &expected_values {
        let patterns = vec![
            format!("0x{:04X}", val),
            format!("0x{:04x}", val),
            format!("{val}"),
        ];
        let mut found = false;
        for pattern in &patterns {
            if output.contains(pattern) {
                found = true;
                break;
            }
        }
        if !found {
            all_found = false;
            log::error!("Value {val} not found in CLI output");
        }
    }

    if !all_found {
        return Err(anyhow!(
            "CLI output does not contain all expected register values"
        ));
    }
    Ok(())
}
