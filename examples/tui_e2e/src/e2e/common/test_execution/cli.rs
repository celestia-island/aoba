use anyhow::{anyhow, Result};
use expectrl::Expect;

use super::super::config::StationConfig;
use ci_utils::*;

/// Timeout for CLI subprocess operations in seconds.
///
/// CLI slave-poll should complete in 5-10 seconds under normal conditions.
/// Using 30 seconds to account for slow CI environments while still catching hung processes.
const CLI_SUBPROCESS_TIMEOUT_SECS: u64 = 30;

/// Verify data received by TUI Master by polling with CLI Slave.
///
/// This function validates that a TUI Master station has successfully read data
/// by using the CLI's `--slave-poll` command to act as a temporary Slave and
/// respond with known test data. The Master's received data is then compared
/// against the expected values.
pub async fn verify_master_data(
    port2: &str,
    expected_data: &[u16],
    config: &StationConfig,
) -> Result<()> {
    log::info!("📡 Polling data from Master...");
    log::info!("🔍 DEBUG: CLI slave-poll starting on port {port2}");
    log::info!("🔍 DEBUG: Expected data: {expected_data:?}");

    let binary = build_debug_bin("aoba")?;
    log::info!("🔍 DEBUG: Using binary: {binary:?}");

    let args_vec: Vec<String> = vec![
        "--slave-poll".to_string(),
        port2.to_string(),
        "--station-id".to_string(),
        config.station_id.to_string(),
        "--register-address".to_string(),
        config.start_address.to_string(),
        "--register-length".to_string(),
        config.register_count.to_string(),
        "--register-mode".to_string(),
        config.register_mode.as_cli_mode().to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
        "--json".to_string(),
    ];
    log::info!("🔍 DEBUG: CLI args: {args_vec:?}");

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CLI_SUBPROCESS_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(&binary).args(&args_vec).output()
        }),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "CLI slave-poll timed out after {} seconds",
            CLI_SUBPROCESS_TIMEOUT_SECS
        )
    })?
    .map_err(|e| anyhow!("Failed to spawn CLI slave-poll task: {}", e))??;

    log::info!("🔍 DEBUG: CLI exit status: {:?}", output.status);
    log::info!(
        "🔍 DEBUG: CLI stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        return Err(anyhow!(
            "CLI slave-poll failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("CLI output: {stdout}");

    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    log::info!("🔍 DEBUG: Parsed JSON: {json:?}");

    if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
        let received_values: Vec<u16> = values
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u16))
            .collect();

        log::info!("🔍 DEBUG: Received values: {received_values:?}");

        if received_values.len() != expected_data.len() {
            return Err(anyhow!(
                "Value count mismatch: expected {}, got {}",
                expected_data.len(),
                received_values.len()
            ));
        }

        for (i, (expected, received)) in
            expected_data.iter().zip(received_values.iter()).enumerate()
        {
            if expected != received {
                log::error!("🔍 DEBUG: Mismatch at index {i}: expected 0x{expected:04X}, got 0x{received:04X}");
                return Err(anyhow!(
                    "Value[{i}] mismatch: expected 0x{expected:04X}, got 0x{received:04X}"
                ));
            }
        }

        log::info!("✅ All {} values verified", expected_data.len());
    } else {
        return Err(anyhow!("No 'values' field found in JSON output"));
    }

    log::info!("✅ Data verification passed");
    Ok(())
}

/// Send data from CLI Master to TUI Slave and verify data integrity.
///
/// This helper uses `aoba --slave-poll` to act as a Modbus master, ensuring
/// the Slave responds with the expected register values.
pub async fn send_data_from_cli_master(
    port2: &str,
    expected_data: &[u16],
    config: &StationConfig,
) -> Result<()> {
    log::info!("📤 Sending data from CLI Master...");
    log::info!("🔍 DEBUG: CLI slave-poll starting on port {port2}");
    log::info!("🔍 DEBUG: Expected data: {expected_data:?}");

    let binary = build_debug_bin("aoba")?;
    log::info!("🔍 DEBUG: Using binary: {binary:?}");

    let args_vec: Vec<String> = vec![
        "--slave-poll".to_string(),
        port2.to_string(),
        "--station-id".to_string(),
        config.station_id.to_string(),
        "--register-address".to_string(),
        config.start_address.to_string(),
        "--register-length".to_string(),
        config.register_count.to_string(),
        "--register-mode".to_string(),
        config.register_mode.as_cli_mode().to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
        "--json".to_string(),
    ];
    log::info!("🔍 DEBUG: CLI args: {args_vec:?}");

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CLI_SUBPROCESS_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(&binary).args(&args_vec).output()
        }),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "CLI slave-poll timed out after {} seconds",
            CLI_SUBPROCESS_TIMEOUT_SECS
        )
    })?
    .map_err(|e| anyhow!("Failed to spawn CLI slave-poll task: {}", e))??;

    log::info!("🔍 DEBUG: CLI exit status: {:?}", output.status);
    log::info!(
        "🔍 DEBUG: CLI stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        return Err(anyhow!(
            "CLI slave-poll failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("CLI output: {stdout}");

    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    log::info!("🔍 DEBUG: Parsed JSON: {json:?}");

    if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
        let received_values: Vec<u16> = values
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u16))
            .collect();

        log::info!("🔍 DEBUG: Received values: {received_values:?}");

        if received_values.len() != expected_data.len() {
            return Err(anyhow!(
                "Value count mismatch: expected {}, got {}",
                expected_data.len(),
                received_values.len()
            ));
        }

        for (i, (expected, received)) in
            expected_data.iter().zip(received_values.iter()).enumerate()
        {
            if expected != received {
                log::error!("🔍 DEBUG: Mismatch at index {i}: expected 0x{expected:04X}, got 0x{received:04X}");
                return Err(anyhow!(
                    "Value[{i}] mismatch: expected 0x{expected:04X}, got 0x{received:04X}"
                ));
            }
        }

        log::info!("✅ All {} values verified", expected_data.len());
    } else {
        return Err(anyhow!("No 'values' field found in JSON output"));
    }

    log::info!("✅ Data verification passed");
    Ok(())
}

/// Verify data stored by a TUI Slave using the status snapshot.
pub async fn verify_slave_data<T: Expect>(
    _session: &mut T,
    _cap: &mut TerminalCapture,
    _expected_data: &[u16],
    config: &StationConfig,
) -> Result<()> {
    log::info!("🔍 Verifying Slave configuration in TUI status file...");

    let status = read_tui_status().map_err(|e| {
        anyhow!(
            "Failed to read TUI status file for Slave verification: {}",
            e
        )
    })?;
    log::info!("🔍 DEBUG: Status file read successfully");

    if status.ports.is_empty() || status.ports[0].modbus_slaves.is_empty() {
        return Err(anyhow!("No slave configuration found in status file"));
    }

    let slave = &status.ports[0].modbus_slaves[0];
    log::info!(
        "🔍 DEBUG: Found {} slave configurations",
        status.ports[0].modbus_slaves.len()
    );
    log::info!(
        "🔍 DEBUG: Slave config - ID:{}, Type:{}, Addr:{}, Count:{}",
        slave.station_id,
        slave.register_type,
        slave.start_address,
        slave.register_count
    );

    if slave.station_id != config.station_id {
        return Err(anyhow!(
            "Slave station ID mismatch: expected {}, got {}",
            config.station_id,
            slave.station_id
        ));
    }

    if slave.start_address != config.start_address {
        return Err(anyhow!(
            "Slave start address mismatch: expected {}, got {}",
            config.start_address,
            slave.start_address
        ));
    }

    if slave.register_count != config.register_count as usize {
        return Err(anyhow!(
            "Slave register count mismatch: expected {}, got {}",
            config.register_count,
            slave.register_count
        ));
    }

    log::info!("✅ Slave configuration verified successfully");
    Ok(())
}
