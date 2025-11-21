use anyhow::Result;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::Stdio,
};

use crate::utils::{build_debug_bin, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2};
use _main::{
    protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode},
    utils::{sleep_1s, sleep_3s},
};

fn write_station_snapshot(file: &mut File, values: &[u16]) -> Result<()> {
    let payload = vec![StationConfig::single_range(
        1,
        StationMode::Master,
        RegisterMode::Holding,
        0,
        values.len() as u16,
        Some(values.to_vec()),
    )];
    let serialized = serde_json::to_string(&payload)?;
    writeln!(file, "{serialized}")?;
    Ok(())
}

/// Test master mode with IPC manual data source
/// This tests that the manual data source mode allows the master to start without external data
pub async fn test_ipc_manual_data_source() -> Result<()> {
    log::info!("🧪 Testing IPC manual data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Start server (Modbus master-provide) with manual data source on port1 in persistent mode
    log::info!(
        "🧪 Starting Modbus server with manual data source on {}...",
        ports.port1_name
    );
    let temp_dir = std::env::temp_dir();
    let server_output = temp_dir.join("server_manual_output.log");
    let server_output_file = File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let mut server = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide-persist",
            &ports.port1_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            "manual",
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Give server time to start
    sleep_3s().await;

    // Check if server is still running
    match server.try_wait()? {
        Some(status) => {
            let stderr = if let Some(stderr) = server.stderr.take() {
                let mut buf = String::new();
                let mut reader = BufReader::new(stderr);
                reader.read_line(&mut buf)?;
                buf
            } else {
                String::new()
            };

            std::fs::remove_file(&server_output).ok();

            return Err(anyhow::anyhow!(
                "Server exited prematurely with status {status}: {stderr}"
            ));
        }
        None => {
            log::info!("✅ Server with manual data source is running");
        }
    }

    // Clean up
    server.kill().ok();
    let _ = server.wait();
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ Manual data source test passed");
    Ok(())
}

/// Test master mode with IPC pipe data source
/// This tests that the IPC pipe data source mode can read data and update continuously
/// Tests 3 rounds of data updates
pub async fn test_ipc_pipe_data_source() -> Result<()> {
    log::info!("🧪 Testing IPC pipe data source mode (3 rounds of data updates)...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Create IPC pipe path - use a regular file for testing since mkfifo needs nix crate
    let ipc_pipe = temp_dir.join("test_ipc_pipe_file");

    // Round 1: Sequential values
    let round1_values: Vec<u16> = vec![0, 1, 2, 3, 4];
    log::info!("📊 Round 1 expected values: {:?}", round1_values);

    // Round 2: Reverse values
    let round2_values: Vec<u16> = vec![4, 3, 2, 1, 0];
    log::info!("📊 Round 2 expected values: {:?}", round2_values);

    // Round 3: Custom hex values
    let round3_values: Vec<u16> = vec![0x1111, 0x2222, 0x3333, 0x4444, 0x5555];
    log::info!("📊 Round 3 expected values: {:?}", round3_values);

    // Create a test file with Round 1 data
    log::info!("🔄 Round 1: Writing sequential values to IPC pipe file");
    {
        let mut file = File::create(&ipc_pipe)?;
        write_station_snapshot(&mut file, &round1_values)?;
    }

    // Start server with IPC pipe data source
    log::info!(
        "🧪 Starting Modbus server with IPC pipe data source on {}...",
        ports.port1_name
    );
    let server_output = temp_dir.join("server_ipc_output.log");
    let server_output_file = File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let mut server = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide-persist",
            &ports.port1_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &format!("ipc:{}", ipc_pipe.display()),
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Give server time to start and read initial data
    sleep_3s().await;

    // Check if server is still running
    match server.try_wait()? {
        Some(status) => {
            let stderr = if let Some(stderr) = server.stderr.take() {
                let mut buf = String::new();
                let mut reader = BufReader::new(stderr);
                reader.read_line(&mut buf)?;
                buf
            } else {
                String::new()
            };

            std::fs::remove_file(&ipc_pipe).ok();
            std::fs::remove_file(&server_output).ok();

            return Err(anyhow::anyhow!(
                "Server exited prematurely with status {status}: {stderr}"
            ));
        }
        None => {
            log::info!("✅ Server with IPC pipe data source is running");
        }
    }

    let binary = build_debug_bin("aoba")?;

    // Test Round 1: Verify sequential values are readable
    log::info!("🔍 Round 1: Polling sequential values from slave");
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
            "5",
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

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        server.kill().ok();
        let _ = server.wait();
        std::fs::remove_file(&ipc_pipe).ok();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow::anyhow!(
            "Round 1: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&client_output.stdout);
    let response_line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Round 1: Client produced empty stdout"))?;
    let response: _main::cli::modbus::ModbusResponse = serde_json::from_str(response_line)?;
    log::info!("✅ Round 1: Received values: {:?}", response.values);

    if response.values != round1_values {
        server.kill().ok();
        let _ = server.wait();
        std::fs::remove_file(&ipc_pipe).ok();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow::anyhow!(
            "Round 1: Received values {:?} do not match expected {:?}",
            response.values,
            round1_values
        ));
    }

    // Test Round 2: Update file with reverse values
    log::info!("🔄 Round 2: Writing reverse values to IPC pipe file");
    {
        let mut file = File::create(&ipc_pipe)?;
        write_station_snapshot(&mut file, &round2_values)?;
    }
    sleep_1s().await;

    log::info!("🔍 Round 2: Polling reverse values from slave");
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
            "5",
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

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        server.kill().ok();
        let _ = server.wait();
        std::fs::remove_file(&ipc_pipe).ok();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow::anyhow!(
            "Round 2: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&client_output.stdout);
    let response_line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Round 2: Client produced empty stdout"))?;
    let response: _main::cli::modbus::ModbusResponse = serde_json::from_str(response_line)?;
    log::info!("✅ Round 2: Received values: {:?}", response.values);

    if response.values != round2_values {
        server.kill().ok();
        let _ = server.wait();
        std::fs::remove_file(&ipc_pipe).ok();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow::anyhow!(
            "Round 2: Received values {:?} do not match expected {:?}",
            response.values,
            round2_values
        ));
    }

    // Test Round 3: Update file with custom hex values
    log::info!("🔄 Round 3: Writing custom hex values to IPC pipe file");
    {
        let mut file = File::create(&ipc_pipe)?;
        write_station_snapshot(&mut file, &round3_values)?;
    }
    sleep_1s().await;

    log::info!("🔍 Round 3: Polling custom hex values from slave");
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
            "5",
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

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        server.kill().ok();
        let _ = server.wait();
        std::fs::remove_file(&ipc_pipe).ok();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow::anyhow!(
            "Round 3: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&client_output.stdout);
    let response_line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Round 3: Client produced empty stdout"))?;
    let response: _main::cli::modbus::ModbusResponse = serde_json::from_str(response_line)?;
    log::info!("✅ Round 3: Received values: {:?}", response.values);

    if response.values != round3_values {
        server.kill().ok();
        let _ = server.wait();
        std::fs::remove_file(&ipc_pipe).ok();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow::anyhow!(
            "Round 3: Received values {:?} do not match expected {:?}",
            response.values,
            round3_values
        ));
    }

    // Clean up
    server.kill().ok();
    let _ = server.wait();
    std::fs::remove_file(&ipc_pipe).ok();
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ IPC pipe data source test passed (all 3 rounds verified)");
    Ok(())
}
