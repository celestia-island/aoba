use anyhow::{anyhow, Result};
use std::{path::PathBuf, process::Stdio};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use aoba::cli::modbus::ModbusResponse;

fn parse_client_response(stdout: &[u8]) -> Result<ModbusResponse> {
    let output = String::from_utf8_lossy(stdout);
    let response_line = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow!("Client produced empty stdout"))?;
    let response: ModbusResponse = serde_json::from_str(response_line)
        .map_err(|err| anyhow!("Failed to parse ModbusResponse JSON: {err}"))?;
    Ok(response)
}

/// Test Python data source with external CPython mode using test_simple.py
pub async fn test_python_data_source_external() -> Result<()> {
    log::info!("🧪 Testing Python data source (external CPython mode)...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Get the path to the test script
    let script_path = PathBuf::from("examples/cli_e2e/scripts/test_simple.py");
    if !script_path.exists() {
        return Err(anyhow!("Test script not found: {}", script_path.display()));
    }

    let script_path_abs = std::fs::canonicalize(&script_path)?;
    log::info!("🧪 Using test script: {}", script_path_abs.display());

    // Expected values from test_simple.py
    let expected_values: Vec<u16> = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];

    let server_output = temp_dir.join("server_python_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let mut master = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide",
            &ports.port1_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &format!("python:external:{}", script_path_abs.display()),
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for master to be ready (minimum 3 seconds)
    wait_for_process_ready(&mut master, 3000).await?;

    let client_output = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--timeout-ms",
            "10000", // 10 second timeout to account for serial communication delays
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    let master_status = master.wait()?;
    if !master_status.success() {
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!("Master exited with status {master_status}"));
    }

    std::fs::remove_file(&server_output).ok();

    if !client_output.status.success() {
        return Err(anyhow!(
            "Slave poll command failed: {} (stderr: {})",
            client_output.status,
            String::from_utf8_lossy(&client_output.stderr)
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    if response.values != expected_values {
        return Err(anyhow!(
            "Received values {:?} do not match expected {:?}",
            response.values,
            expected_values
        ));
    }

    log::info!("✅ Python data source (external) test passed");
    Ok(())
}

/// Test Python data source with external CPython mode in persistent mode
pub async fn test_python_data_source_external_persist() -> Result<()> {
    log::info!("🧪 Testing Python data source persistent mode (external CPython)...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Get the path to the test script
    let script_path = PathBuf::from("examples/cli_e2e/scripts/test_simple.py");
    if !script_path.exists() {
        return Err(anyhow!("Test script not found: {}", script_path.display()));
    }

    let script_path_abs = std::fs::canonicalize(&script_path)?;
    log::info!("🧪 Using test script: {}", script_path_abs.display());

    // Expected values from test_simple.py
    let expected_values: Vec<u16> = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];

    let server_output = temp_dir.join("server_python_persist_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let mut master = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide-persist",
            &ports.port1_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &format!("python:external:{}", script_path_abs.display()),
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for master to be ready (minimum 3 seconds)
    wait_for_process_ready(&mut master, 3000).await?;

    let client_output = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--timeout-ms",
            "10000", // 10 second timeout to account for serial communication delays
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    if response.values != expected_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Received values {:?} do not match expected {:?}",
            response.values,
            expected_values
        ));
    }

    // Verify master is still running (persistent mode)
    if let Some(status) = master.try_wait()? {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Master exited after handling poll with status {status}"
        ));
    }

    master.kill().ok();
    let _ = master.wait();
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ Python data source persistent test passed");
    Ok(())
}

/// Test Python data source with dynamic data generation (test_dynamic.py)
pub async fn test_python_data_source_dynamic() -> Result<()> {
    log::info!("🧪 Testing Python data source with dynamic data...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Get the path to the test script
    let script_path = PathBuf::from("examples/cli_e2e/scripts/test_dynamic.py");
    if !script_path.exists() {
        return Err(anyhow!("Test script not found: {}", script_path.display()));
    }

    let script_path_abs = std::fs::canonicalize(&script_path)?;
    log::info!("🧪 Using test script: {}", script_path_abs.display());

    let server_output = temp_dir.join("server_python_dynamic_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let mut master = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide",
            &ports.port1_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "3", // Only 3 registers for dynamic test (temperature, humidity, pressure)
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &format!("python:external:{}", script_path_abs.display()),
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for master to be ready
    wait_for_process_ready(&mut master, 3000).await?;

    let client_output = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "3",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--timeout-ms",
            "10000",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    let master_status = master.wait()?;
    if !master_status.success() {
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!("Master exited with status {master_status}"));
    }

    std::fs::remove_file(&server_output).ok();

    if !client_output.status.success() {
        return Err(anyhow!(
            "Slave poll command failed: {} (stderr: {})",
            client_output.status,
            String::from_utf8_lossy(&client_output.stderr)
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;

    // Verify we got 3 values
    if response.values.len() != 3 {
        return Err(anyhow!("Expected 3 values, got {}", response.values.len()));
    }

    // Verify values are in expected ranges (from test_dynamic.py)
    let temperature = response.values[0];
    let humidity = response.values[1];
    let pressure = response.values[2];

    if !(200..=300).contains(&temperature) {
        return Err(anyhow!(
            "Temperature {} out of expected range 200-300",
            temperature
        ));
    }
    if !(400..=600).contains(&humidity) {
        return Err(anyhow!(
            "Humidity {} out of expected range 400-600",
            humidity
        ));
    }
    if !(9800..=10200).contains(&pressure) {
        return Err(anyhow!(
            "Pressure {} out of expected range 9800-10200",
            pressure
        ));
    }

    log::info!(
        "✅ Python data source dynamic test passed (T={}, H={}, P={})",
        temperature,
        humidity,
        pressure
    );
    Ok(())
}
