use anyhow::{anyhow, ensure, Context, Result};
use once_cell::sync::OnceCell;
use serde_json::json;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::{Builder, TempPath};

#[cfg(unix)]
use expectrl::Expect;

use super::super::config::{RegisterModeExt, StationConfig};
use aoba_ci_utils::*;

static AOBA_BINARY: OnceCell<PathBuf> = OnceCell::new();

fn workspace_root() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    current_dir
        .ancestors()
        .find(|p| {
            let cargo_toml = p.join("Cargo.toml");
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                content.contains("[workspace]") || content.contains("name = \"aoba\"")
            } else {
                false
            }
        })
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("Could not locate workspace root"))
}

fn ensure_aoba_binary() -> Result<&'static PathBuf> {
    AOBA_BINARY.get_or_try_init(|| {
        let root = workspace_root()?;
        build_aoba_debug_binary(&root)?;
        resolve_aoba_binary_path(&root)
    })
}

fn build_aoba_debug_binary(root: &Path) -> Result<()> {
    log::info!("⚙️ Building aoba debug binary for TUI E2E harness");
    let status = Command::new("cargo")
        .arg("build")
        .arg("--bin")
        .arg("aoba")
        .current_dir(root)
        .status()
        .context("Failed to execute `cargo build --bin aoba`")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "`cargo build --bin aoba` exited with status {status}",
            status = status
        ))
    }
}

fn resolve_aoba_binary_path(root: &Path) -> Result<PathBuf> {
    let exe_name = if cfg!(windows) { "aoba.exe" } else { "aoba" };
    let candidates = [
        root.join("target").join("debug").join(exe_name),
        root.join("target").join("release").join(exe_name),
    ];

    if let Some(binary) = candidates.iter().find(|candidate| candidate.exists()) {
        log::info!("✅ Using aoba binary at {}", binary.display());
        Ok(binary.to_path_buf())
    } else {
        let searched = candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Err(anyhow!(
            "aoba binary not found in target directories after build (searched: {searched})"
        ))
    }
}

fn prepare_file_payload(expected_data: &[u16]) -> Result<(String, TempPath)> {
    let payload = json!({ "values": expected_data }).to_string();
    let mut temp_file = Builder::new()
        .prefix("tui_slave_payload_")
        .suffix(".jsonl")
        .tempfile()
        .context("Failed to create temporary payload file")?;

    {
        let file = temp_file.as_file_mut();
        writeln!(file, "{payload}").context("Failed to write payload to temporary file")?;
        file.flush()
            .context("Failed to flush temporary payload file")?;
    }

    let temp_path = temp_file.into_temp_path();
    let data_source = format!("file:{}", temp_path.display());
    Ok((data_source, temp_path))
}

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

    ensure!(
        config.is_single_range(),
        "Station {} defines {} register ranges; CLI single-range helpers only support one range. Use a JSON config file with --config for multi-range or multi-station scenarios.",
        config.station_id,
        config.range_count()
    );

    let binary = ensure_aoba_binary()?;
    log::info!("🔍 DEBUG: Using binary: {}", binary.display());

    let args_vec: Vec<String> = vec![
        "--slave-poll".to_string(),
        port2.to_string(),
        "--station-id".to_string(),
        config.station_id.to_string(),
        "--register-address".to_string(),
        config.start_address().to_string(),
        "--register-length".to_string(),
        config.register_count().to_string(),
        "--register-mode".to_string(),
        config.register_mode().as_cli_mode().to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
        "--json".to_string(),
    ];
    log::info!("🔍 DEBUG: CLI args: {args_vec:?}");

    let binary_path = binary.clone();
    let args_clone = args_vec.clone();

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CLI_SUBPROCESS_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(&binary_path)
                .args(&args_clone)
                .output()
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

/// Provide register data for the TUI Slave by acting as the remote Modbus device.
///
/// When the TUI runs in **Slave mode** it internally spawns `aoba --slave-poll-persist`
/// to behave as a Modbus **master/client**, issuing read requests on `port2`. To
/// complete the round trip in tests we launch a matching `aoba --master-provide`
/// process on the paired port so that the TUI receives deterministic register
/// values. The helper blocks until one request is served and then validates the
/// CLI's JSON output matches `expected_data`.
pub async fn send_data_from_cli_master(
    port2: &str,
    expected_data: &[u16],
    config: &StationConfig,
) -> Result<()> {
    log::info!("📤 Providing data to TUI Slave via CLI master-provide...");
    log::info!("🔍 DEBUG: CLI master-provide starting on port {port2}");
    log::info!("🔍 DEBUG: Expected data: {expected_data:?}");

    ensure!(
        config.is_single_range(),
        "Station {} defines {} register ranges; CLI single-range helpers only support one range. Use a JSON config file with --config for multi-range or multi-station scenarios.",
        config.station_id,
        config.range_count()
    );

    let binary = ensure_aoba_binary()?;
    log::info!("🔍 DEBUG: Using binary: {}", binary.display());

    let (data_source, temp_path_guard) = prepare_file_payload(expected_data)?;
    let _temp_path_guard = temp_path_guard;

    log::info!("🔍 DEBUG: Data source prepared at {data_source}");

    let args_vec: Vec<String> = vec![
        "--master-provide".to_string(),
        port2.to_string(),
        "--station-id".to_string(),
        config.station_id.to_string(),
        "--register-address".to_string(),
        config.start_address().to_string(),
        "--register-length".to_string(),
        config.register_count().to_string(),
        "--register-mode".to_string(),
        config.register_mode().as_cli_mode().to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
        "--data-source".to_string(),
        data_source,
        "--json".to_string(),
    ];
    log::info!("🔍 DEBUG: CLI args: {args_vec:?}");

    let binary_path = binary.clone();
    let args_clone = args_vec.clone();

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CLI_SUBPROCESS_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(&binary_path)
                .args(&args_clone)
                .output()
        }),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "CLI master-provide timed out after {} seconds",
            CLI_SUBPROCESS_TIMEOUT_SECS
        )
    })?
    .map_err(|e| anyhow!("Failed to spawn CLI master-provide task: {}", e))??;

    log::info!("🔍 DEBUG: CLI exit status: {:?}", output.status);
    log::info!(
        "🔍 DEBUG: CLI stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        return Err(anyhow!(
            "CLI master-provide failed: {}",
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

    ensure!(
        config.is_single_range(),
        "Station {} defines {} register ranges; CLI single-range helpers only support one range. Use a JSON config file with --config for multi-range or multi-station scenarios.",
        config.station_id,
        config.range_count()
    );

    let status = read_tui_status().map_err(|e| {
        anyhow!(
            "Failed to read TUI status file for Slave verification: {}",
            e
        )
    })?;
    log::info!("🔍 DEBUG: Status file read successfully");

    let slaves: Vec<_> = status
        .ports
        .iter()
        .flat_map(|port| port.modbus_slaves.iter())
        .collect();

    ensure!(
        !slaves.is_empty(),
        "No slave configuration found in status file. CLI helper expects exactly one slave; multi-station tests must use a config file instead."
    );

    ensure!(
        slaves.len() == 1,
        "Found {} slave configurations in status file; CLI single-range helpers support only one. Use a JSON config file with --config for multi-station scenarios.",
        slaves.len()
    );

    let slave = slaves[0];
    log::info!("🔍 DEBUG: Found {} slave configurations", slaves.len());
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

    if slave.start_address != config.start_address() {
        return Err(anyhow!(
            "Slave start address mismatch: expected {}, got {}",
            config.start_address(),
            slave.start_address
        ));
    }

    if slave.register_count != config.register_count() as usize {
        return Err(anyhow!(
            "Slave register count mismatch: expected {}, got {}",
            config.register_count(),
            slave.register_count
        ));
    }

    log::info!("✅ Slave configuration verified successfully");
    Ok(())
}
