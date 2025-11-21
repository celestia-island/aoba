use anyhow::{anyhow, Result};
use std::{
    io::{BufRead, BufReader, Write},
    process::Stdio,
    sync::Arc,
};

use interprocess::local_socket::{prelude::*, Stream as LocalStream};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use _main::{
    cli::modbus::ModbusResponse,
    protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode},
    utils::{sleep::sleep_1s, sleep_3s},
};

// File-level constants to avoid magic numbers
const REGISTER_LENGTH: usize = 10;

// Use platform-specific IPC socket path
#[cfg(windows)]
const IPC_SOCKET_PATH: &str = "aoba_test_ipc_channel";

#[cfg(not(windows))]
const IPC_SOCKET_PATH: &str = "/tmp/aoba_test_ipc_channel.sock";

/// Build station payload for data transmission
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

/// Test IPC channel data source - master with IPC socket, E2E sends data via IPC, slave polls master
/// Tests 3 rounds of IPC write followed by slave poll verification
pub async fn test_ipc_channel_data_source() -> Result<()> {
    log::info!(
        "🧪 Testing IPC channel data source mode (master with IPC, E2E as client, slave polls)..."
    );
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

    // Remove old socket file if exists (Unix only)
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(IPC_SOCKET_PATH);
    }

    // Start master daemon with IPC socket on vcom1
    let master_output = temp_dir.join("master_ipc_persist_output.log");
    let master_output_file = std::fs::File::create(&master_output)?;
    let master_stderr = temp_dir.join("master_ipc_persist_stderr.log");
    let master_stderr_file = std::fs::File::create(&master_stderr)?;

    log::info!(
        "📋 Master logs will be at: stdout={:?}, stderr={:?}",
        master_output,
        master_stderr
    );

    log::info!(
        "🚀 Starting master daemon with IPC socket on {}",
        ports.port1_name
    );

    let mut master = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide-persist",
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
            "--data-source",
            "manual",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::from(master_output_file))
        .stderr(Stdio::from(master_stderr_file))
        .spawn()?;

    // Write initial empty data to stdin to initialize master
    // Note: stdin must be kept open for persist mode, so we don't drop it
    let mut _master_stdin = master.stdin.take();
    if let Some(ref mut stdin) = _master_stdin {
        let empty_payload = build_station_payload(&[0u16; REGISTER_LENGTH]);
        let json = serde_json::to_string(&*empty_payload)?;
        writeln!(stdin, "{}", json)?;
        stdin.flush()?;
    }

    // Wait for master to be ready and create IPC socket
    wait_for_process_ready(&mut master, 3000).await?;
    log::info!("⏳ Waiting for IPC socket to be created...");
    sleep_3s().await;

    // Helper function to send data via IPC with retry
    let send_data_via_ipc = |values: &[u16]| -> Result<()> {
        let payload = build_station_payload(values);
        let json = serde_json::to_string(&*payload)?;

        // Wait for IPC socket to be created (with retry)
        let mut retries = 20;
        let stream = loop {
            // Use filesystem-based socket path (matching the server implementation)
            let connect_result = IPC_SOCKET_PATH
                .to_fs_name::<interprocess::local_socket::GenericFilePath>()
                .and_then(LocalStream::connect);

            match connect_result {
                Ok(s) => break s,
                Err(_e) if retries > 0 => {
                    log::debug!("Waiting for IPC socket... ({} retries left)", retries);
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    retries -= 1;
                }
                Err(e) => return Err(anyhow!("Failed to connect to IPC socket: {}", e)),
            }
        };

        // Send data
        let mut stream = stream;
        writeln!(stream, "{}", json)?;
        stream.flush()?;

        // Read response
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        let response_json: serde_json::Value = serde_json::from_str(response.trim())?;
        if !response_json["success"].as_bool().unwrap_or(false) {
            return Err(anyhow!("IPC write failed: {:?}", response_json));
        }

        Ok(())
    };

    // Test Round 1: Sequential values
    log::info!("🔄 Round 1: Sending sequential values via IPC");
    send_data_via_ipc(&round1_values)?;
    sleep_1s().await;

    // Verify by polling master with slave
    log::info!("🔍 Round 1: Polling master with slave to verify");
    let poll_output = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
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
        ])
        .output()?;

    if !poll_output.status.success() {
        let stderr = String::from_utf8_lossy(&poll_output.stderr);
        master.kill().ok();
        let _ = master.wait();
        return Err(anyhow!(
            "Round 1: Slave poll failed: {} (stderr: {})",
            poll_output.status,
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&poll_output.stdout);
    let response: ModbusResponse = serde_json::from_str(stdout.trim())?;
    log::info!("✅ Round 1: Polled values: {:?}", response.values);

    if response.values != round1_values {
        master.kill().ok();
        let _ = master.wait();
        return Err(anyhow!(
            "Round 1: Received values {:?} do not match expected {:?}",
            response.values,
            round1_values
        ));
    }

    // Test Round 2: Reverse values
    log::info!("🔄 Round 2: Sending reverse values via IPC");
    send_data_via_ipc(&round2_values)?;
    sleep_1s().await;

    log::info!("🔍 Round 2: Polling master with slave to verify");
    let poll_output2 = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
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
        ])
        .output()?;

    if !poll_output2.status.success() {
        let stderr = String::from_utf8_lossy(&poll_output2.stderr);
        master.kill().ok();
        let _ = master.wait();
        return Err(anyhow!(
            "Round 2: Slave poll failed: {} (stderr: {})",
            poll_output2.status,
            stderr
        ));
    }

    let stdout2 = String::from_utf8_lossy(&poll_output2.stdout);
    let response2: ModbusResponse = serde_json::from_str(stdout2.trim())?;
    log::info!("✅ Round 2: Polled values: {:?}", response2.values);

    if response2.values != round2_values {
        master.kill().ok();
        let _ = master.wait();
        return Err(anyhow!(
            "Round 2: Received values {:?} do not match expected {:?}",
            response2.values,
            round2_values
        ));
    }

    // Test Round 3: Custom hex values
    log::info!("🔄 Round 3: Sending custom hex values via IPC");
    send_data_via_ipc(&round3_values)?;
    sleep_1s().await;

    log::info!("🔍 Round 3: Polling master with slave to verify");
    let poll_output3 = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
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
        ])
        .output()?;

    if !poll_output3.status.success() {
        let stderr = String::from_utf8_lossy(&poll_output3.stderr);
        master.kill().ok();
        let _ = master.wait();
        return Err(anyhow!(
            "Round 3: Slave poll failed: {} (stderr: {})",
            poll_output3.status,
            stderr
        ));
    }

    let stdout3 = String::from_utf8_lossy(&poll_output3.stdout);
    let response3: ModbusResponse = serde_json::from_str(stdout3.trim())?;
    log::info!("✅ Round 3: Polled values: {:?}", response3.values);

    if response3.values != round3_values {
        master.kill().ok();
        let _ = master.wait();
        return Err(anyhow!(
            "Round 3: Received values {:?} do not match expected {:?}",
            response3.values,
            round3_values
        ));
    }

    // Cleanup
    master.kill().ok();
    let _ = master.wait();

    // Remove socket file (Unix only)
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(IPC_SOCKET_PATH);
    }

    std::fs::remove_file(&master_output).ok();

    log::info!("✅ IPC channel data source test completed successfully");
    Ok(())
}
