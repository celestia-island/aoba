use anyhow::Result;
use std::process::Stdio;

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use aoba::cli::modbus::ModbusResponse;

/// Test master mode with Python script data source
pub async fn test_python_script_data_source() -> Result<()> {
    log::info!("🧪 Testing Python script data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Path to the Python script
    let script_path = std::path::Path::new("scripts/modbus_data_source.py");
    let script_path = std::fs::canonicalize(script_path)?;
    let script_path_str = format!("python:{}", script_path.to_str().unwrap());

    log::info!("🧪 Using Python script data source: {}", script_path_str);

    // Clean up counter file from previous runs
    let counter_file = "/tmp/modbus_data_source_counter.txt";
    let _ = std::fs::remove_file(counter_file);

    let binary = build_debug_bin("aoba")?;

    // Round 1: Sequential 0-9
    log::info!("🧪 Testing Round 1: Sequential 0-9");
    let expected_round1 = vec![0u16, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    test_python_round(&binary, &script_path_str, &ports, &expected_round1).await?;

    // Round 2: Reverse 9-0
    log::info!("🧪 Testing Round 2: Reverse 9-0");
    let expected_round2 = vec![9u16, 8, 7, 6, 5, 4, 3, 2, 1, 0];
    test_python_round(&binary, &script_path_str, &ports, &expected_round2).await?;

    // Round 3: Custom pattern
    log::info!("🧪 Testing Round 3: Custom pattern");
    let expected_round3 = vec![
        0x1111u16, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888, 0x9999, 0xAAAA,
    ];
    test_python_round(&binary, &script_path_str, &ports, &expected_round3).await?;

    log::info!("✅ Python script data source test completed successfully");
    Ok(())
}

async fn test_python_round(
    binary: &std::path::Path,
    data_source: &str,
    ports: &crate::utils::VcomMatchers,
    expected: &[u16],
) -> Result<()> {
    let temp_dir = std::env::temp_dir();
    let server_output = temp_dir.join(format!("server_python_output_{}.log", std::process::id()));
    let server_output_file = std::fs::File::create(&server_output)?;
    
    let server_stderr = temp_dir.join(format!("server_python_stderr_{}.log", std::process::id()));
    let server_stderr_file = std::fs::File::create(&server_stderr)?;

    // Start master with Python script data source
    let mut master = std::process::Command::new(binary)
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
            data_source,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::from(server_stderr_file))
        .spawn()?;

    // Wait for master to be ready and Python script to execute
    let ready_result = wait_for_process_ready(&mut master, 3000).await;
    
    // If failed to start, log stderr for debugging
    if ready_result.is_err() {
        if let Ok(stderr_content) = std::fs::read_to_string(&server_stderr) {
            log::error!("❌ Master stderr:\n{}", stderr_content);
        }
        if let Ok(stdout_content) = std::fs::read_to_string(&server_output) {
            log::error!("❌ Master stdout:\n{}", stdout_content);
        }
        ready_result?;
    }

    // Poll slave to get data
    let client_output = std::process::Command::new(binary)
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
            "10000", // 10 second timeout
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    // Kill master for this round
    master.kill()?;

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        log::error!("❌ Slave poll failed:\n{}", stderr);
        anyhow::bail!("Slave poll command failed");
    }

    let stdout = String::from_utf8_lossy(&client_output.stdout);
    log::debug!("Raw slave response:\n{}", stdout);

    let response: ModbusResponse = serde_json::from_str(stdout.trim()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse JSON response: {}\nRaw output: {}",
            e,
            stdout
        )
    })?;

    log::info!("📊 Received registers: {:?}", response.values);

    if response.values != expected {
        anyhow::bail!(
            "Register mismatch!\nExpected: {:?}\nActual: {:?}",
            expected,
            response.values
        );
    }

    log::info!("✅ Round completed successfully");
    Ok(())
}
