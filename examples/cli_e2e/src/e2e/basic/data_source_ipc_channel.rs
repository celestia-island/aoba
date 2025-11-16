use anyhow::{anyhow, Result};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    process::Stdio,
    sync::Arc,
};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use _main::{
    cli::modbus::ModbusResponse,
    protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode},
    utils::sleep::sleep_1s,
};

// File-level constants to avoid magic numbers
const REGISTER_LENGTH: usize = 10;
const IPC_SOCKET_PATH: &str = "/tmp/aoba_test_ipc_channel.sock";

/// Send a JSON request to the IPC socket and receive response
fn send_ipc_request(socket_path: &str, request: &str) -> Result<String> {
    // Remove existing socket file if any
    let _ = std::fs::remove_file(socket_path);

    // Wait for server to be ready
    let mut retries = 10;
    let mut stream = None;
    while retries > 0 {
        match UnixStream::connect(socket_path) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(500));
                retries -= 1;
            }
        }
    }

    let mut stream = stream.ok_or_else(|| anyhow!("Failed to connect to IPC socket"))?;

    // Send request
    writeln!(stream, "{}", request)?;
    stream.flush()?;

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    Ok(response.trim().to_string())
}

/// Parse IPC response
fn parse_ipc_response(response: &str) -> Result<ModbusResponse> {
    let json: serde_json::Value = serde_json::from_str(response)?;

    if !json["success"].as_bool().unwrap_or(false) {
        return Err(anyhow!("IPC request failed: {:?}", json["error"]));
    }

    let data = &json["data"];
    let response: ModbusResponse = serde_json::from_value(data.clone())?;
    Ok(response)
}

/// Build station payload for writing data
fn build_station_payload(values: &[u16]) -> Arc<Vec<StationConfig>> {
    Arc::new(vec![StationConfig::single_range(
        1,
        StationMode::Master,
        RegisterMode::Holding,
        0,
        REGISTER_LENGTH as u16,
        Some(values.to_vec()),
    )])
}

/// Test IPC channel data source - master writes, slave provides data via IPC
/// Tests 3 rounds of write-read cycles to verify register operations
pub async fn test_ipc_channel_data_source() -> Result<()> {
    log::info!("🧪 Testing IPC channel data source mode (3 rounds of write-read cycles)...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Round 1: Sequential values
    let round1_values: Vec<u16> = (0..REGISTER_LENGTH as u16).collect();
    log::info!("📊 Round 1 expected values: {:?}", round1_values);

    // Round 2: Reverse values
    let round2_values: Vec<u16> = (0..REGISTER_LENGTH as u16).rev().collect();
    log::info!("📊 Round 2 expected values: {:?}", round2_values);

    // Round 3: Custom hex values
    let round3_values: Vec<u16> = vec![
        0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888, 0x9999, 0xAAAA,
    ];
    log::info!("📊 Round 3 expected values: {:?}", round3_values);

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

    // Start slave with IPC socket
    let slave_output = temp_dir.join("slave_ipc_persist_output.log");
    let slave_output_file = std::fs::File::create(&slave_output)?;
    let slave_stderr = temp_dir.join("slave_ipc_persist_stderr.log");
    let slave_stderr_file = std::fs::File::create(&slave_stderr)?;

    log::info!(
        "📋 Slave logs will be at: stdout={:?}, stderr={:?}",
        slave_output,
        slave_stderr
    );

    // Remove old socket file if exists
    let _ = std::fs::remove_file(IPC_SOCKET_PATH);

    let mut slave = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-listen-persist",
            &ports.port1_name,
            "--ipc-socket-path",
            IPC_SOCKET_PATH,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
        ])
        .stdout(Stdio::from(slave_output_file))
        .stderr(Stdio::from(slave_stderr_file))
        .spawn()?;

    // Wait for slave to be ready
    wait_for_process_ready(&mut slave, 3000).await?;
    sleep_1s().await;

    // Test Round 1: Sequential values
    log::info!("🔄 Round 1: Writing sequential values via master");
    let payload_round1 = build_station_payload(&round1_values);

    let mut master_proc = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            "manual",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Write data to stdin
    if let Some(mut stdin) = master_proc.stdin.take() {
        let json = serde_json::to_string(&*payload_round1)?;
        writeln!(stdin, "{}", json)?;
        stdin.flush()?;
        drop(stdin);
    }

    let output = master_proc.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        slave.kill().ok();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 1: Master write command failed: {} (stderr: {})",
            output.status,
            stderr
        ));
    }

    sleep_1s().await;

    // Read data via IPC
    log::info!("🔍 Round 1: Reading data via IPC channel");
    let ipc_response = send_ipc_request(IPC_SOCKET_PATH, r#"{"action":"read"}"#)?;
    let response = parse_ipc_response(&ipc_response)?;
    log::info!("✅ Round 1: Received values: {:?}", response.values);

    if response.values != round1_values {
        slave.kill().ok();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 1: Received values {:?} do not match expected {:?}",
            response.values,
            round1_values
        ));
    }

    // Test Round 2: Reverse values
    log::info!("🔄 Round 2: Writing reverse values via master");
    let payload_round2 = build_station_payload(&round2_values);

    let mut master_proc = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            "manual",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = master_proc.stdin.take() {
        let json = serde_json::to_string(&*payload_round2)?;
        writeln!(stdin, "{}", json)?;
        stdin.flush()?;
        drop(stdin);
    }

    let output = master_proc.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        slave.kill().ok();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 2: Master write command failed: {} (stderr: {})",
            output.status,
            stderr
        ));
    }

    sleep_1s().await;

    log::info!("🔍 Round 2: Reading data via IPC channel");
    let ipc_response = send_ipc_request(IPC_SOCKET_PATH, r#"{"action":"read"}"#)?;
    let response = parse_ipc_response(&ipc_response)?;
    log::info!("✅ Round 2: Received values: {:?}", response.values);

    if response.values != round2_values {
        slave.kill().ok();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 2: Received values {:?} do not match expected {:?}",
            response.values,
            round2_values
        ));
    }

    // Test Round 3: Custom hex values
    log::info!("🔄 Round 3: Writing custom hex values via master");
    let payload_round3 = build_station_payload(&round3_values);

    let mut master_proc = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            "manual",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = master_proc.stdin.take() {
        let json = serde_json::to_string(&*payload_round3)?;
        writeln!(stdin, "{}", json)?;
        stdin.flush()?;
        drop(stdin);
    }

    let output = master_proc.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        slave.kill().ok();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 3: Master write command failed: {} (stderr: {})",
            output.status,
            stderr
        ));
    }

    sleep_1s().await;

    log::info!("🔍 Round 3: Reading data via IPC channel");
    let ipc_response = send_ipc_request(IPC_SOCKET_PATH, r#"{"action":"read"}"#)?;
    let response = parse_ipc_response(&ipc_response)?;
    log::info!("✅ Round 3: Received values: {:?}", response.values);

    if response.values != round3_values {
        slave.kill().ok();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 3: Received values {:?} do not match expected {:?}",
            response.values,
            round3_values
        ));
    }

    // Cleanup
    slave.kill().ok();
    let _ = slave.wait();
    let _ = std::fs::remove_file(IPC_SOCKET_PATH);
    std::fs::remove_file(&slave_output).ok();

    log::info!("✅ IPC channel data source test completed successfully");
    Ok(())
}
