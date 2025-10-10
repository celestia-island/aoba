use anyhow::{anyhow, Result};

use ci_utils::run_binary_sync;

pub async fn test_cli_list_ports_json_with_status() -> Result<()> {
    let output = run_binary_sync(&["--list-ports", "--json"])?;

    log::info!(
        "list-ports-json-status stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    log::info!(
        "list-ports-json-status stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        log::error!(
            "List ports JSON command failed with status: {}",
            output.status
        );
        return Err(anyhow!(
            "List ports JSON command failed with status: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that output contains JSON array
    if !stdout.starts_with('[') && !stdout.contains(']') {
        return Err(anyhow!("Expected JSON array output, got: {stdout}"));
    }

    // Parse JSON to verify structure
    let json: serde_json::Value = serde_json::from_str(&stdout)?;

    if let Some(arr) = json.as_array() {
        if arr.is_empty() {
            log::warn!("No serial ports found");
        } else {
            // Check first port has required fields
            if let Some(port) = arr.first() {
                if port.get("path").is_none() {
                    return Err(anyhow!("Port missing 'path' field"));
                }
                if port.get("status").is_none() {
                    return Err(anyhow!("Port missing 'status' field"));
                }

                let status = port.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if status != "Free" && status != "Occupied" {
                    return Err(anyhow!("Invalid status value: {status}"));
                }

                log::info!("🧪 Port status field verified: {status}");
            }
        }
    } else {
        return Err(anyhow!("Expected JSON array, got different type"));
    }

    log::info!("🧪 JSON output with status field works correctly");
    Ok(())
}
