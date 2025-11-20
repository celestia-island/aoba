use anyhow::{anyhow, Result};
use std::{
    io::{BufRead, BufReader, Write},
    process::Stdio,
    sync::Arc,
};

use interprocess::local_socket::{prelude::*, GenericNamespaced, Stream as LocalStream};

use crate::utils::{build_debug_bin, wait_for_process_ready};
use _main::{
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

/// Test IPC channel data source - master with IPC socket using virtual port (UUID)
/// Tests 3 rounds of IPC write followed by verification
/// Virtual ports use UUID as port name and expose IPC communication (no baud rate needed)
pub async fn test_ipc_channel_data_source() -> Result<()> {
    log::info!(
        "🧪 Testing IPC channel data source mode (master with IPC, E2E as client, virtual port)..."
    );
    let temp_dir = std::env::temp_dir();

    // Generate UUID v7 for virtual port
    let virtual_port_uuid = uuid::Uuid::now_v7().to_string();
    log::info!("📍 Using virtual port UUID: {}", virtual_port_uuid);

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

    // Start master daemon with IPC socket using virtual port (UUID)
    // Virtual ports automatically use IPC/HTTP, no baud rate needed
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
        "🚀 Starting master daemon with virtual port {} and IPC socket",
        virtual_port_uuid
    );

    let mut master = std::process::Command::new(&binary)
        .args([
            "--master-provide-persist",
            &virtual_port_uuid,
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
            // Note: No baud rate for virtual ports - it's irrelevant
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
            // Try namespaced socket first (matching server logic), then filesystem
            let connect_result = if let Ok(ns) = IPC_SOCKET_PATH.to_ns_name::<GenericNamespaced>() {
                LocalStream::connect(ns)
            } else {
                IPC_SOCKET_PATH
                    .to_fs_name::<interprocess::local_socket::GenericFilePath>()
                    .and_then(LocalStream::connect)
            };

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
    log::info!("✅ Round 1: Successfully sent values via IPC to virtual port");
    sleep_1s().await;

    // Test Round 2: Reverse values
    log::info!("🔄 Round 2: Sending reverse values via IPC");
    send_data_via_ipc(&round2_values)?;
    log::info!("✅ Round 2: Successfully sent values via IPC to virtual port");
    sleep_1s().await;

    // Test Round 3: Custom hex values
    log::info!("🔄 Round 3: Sending custom hex values via IPC");
    send_data_via_ipc(&round3_values)?;
    log::info!("✅ Round 3: Successfully sent values via IPC to virtual port");
    sleep_1s().await;

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
