use anyhow::{anyhow, Result};
use std::{process::Stdio, sync::Arc};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use _main::{
    api::modbus::ModbusResponse,
    protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode},
    utils::sleep::{sleep_1s, sleep_3s},
};

// File-level constants to avoid magic numbers
const REGISTER_LENGTH: usize = 10;
const HTTP_SERVER_PORT_DEFAULT: u16 = 8080;

/// Post JSON data to the HTTP server running in the subprocess
async fn post_data_to_server(port: u16, payload: Arc<Vec<StationConfig>>) -> Result<()> {
    // Named constants to avoid magic numbers and make retry policy explicit
    const HTTP_POST_RETRIES: usize = 3;
    let url = format!("http://127.0.0.1:{}", port);

    // Wait a bit for the server to start
    sleep_3s().await;

    // Try to POST the data with retries
    for attempt in 0..HTTP_POST_RETRIES {
        match ureq::post(&url).send_json(&*payload) {
            Ok(resp) if resp.status() == 200 => {
                log::info!(
                    "Successfully posted data to HTTP server on attempt {}",
                    attempt + 1
                );
                return Ok(());
            }
            Ok(resp) => {
                log::warn!("HTTP POST returned status {}, retrying...", resp.status());
            }
            Err(err) => {
                log::warn!(
                    "Failed to POST to HTTP server (attempt {}): {}",
                    attempt + 1,
                    err
                );
            }
        }
        sleep_1s().await;
    }

    Err(anyhow!(
        "Failed to POST data to HTTP server after {} attempts",
        HTTP_POST_RETRIES
    ))
}

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

/// Test master mode with HTTP data source - master runs HTTP server in persistent mode, test POSTs data
/// Tests 3 rounds of data updates to verify continuous data reception
pub async fn test_http_data_source() -> Result<()> {
    log::info!("🧪 Testing HTTP data source mode (3 rounds of data updates)...");
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

    let payload_round1 = build_station_payload(&round1_values);

    // Use a fixed port for the HTTP server that the master will run
    let http_port = HTTP_SERVER_PORT_DEFAULT;
    let data_source_arg = format!("http://{}", http_port);

    log::info!(
        "🧪 Master will run HTTP server on port {}",
        HTTP_SERVER_PORT_DEFAULT
    );

    let server_output = temp_dir.join("server_http_persist_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;
    let server_stderr = temp_dir.join("server_http_persist_stderr.log");
    let server_stderr_file = std::fs::File::create(&server_stderr)?;

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

    log::info!(
        "📋 Master logs will be at: stdout={:?}, stderr={:?}",
        server_output,
        server_stderr
    );

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
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &data_source_arg,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::from(server_stderr_file))
        .spawn()?;

    // Wait for master to be ready with flexible waiting (minimum 3 seconds)
    wait_for_process_ready(&mut master, 3000).await?;

    // Test Round 1: Sequential values
    log::info!("🔄 Round 1: Posting sequential values to HTTP server");
    let payload_clone = payload_round1.clone();
    let post_handle =
        tokio::spawn(async move { post_data_to_server(http_port, payload_clone).await });
    post_handle.await??;
    sleep_1s().await;

    log::info!("🔍 Round 1: Polling data from slave");
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
            &register_length_arg,
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
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Round 1: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    log::info!("✅ Round 1: Received values: {:?}", response.values);
    if response.values != round1_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Round 1: Received values {:?} do not match expected {:?}",
            response.values,
            round1_values
        ));
    }

    // Test Round 2: Reverse values
    log::info!("🔄 Round 2: Posting reverse values to HTTP server");
    let payload_round2 = build_station_payload(&round2_values);
    let payload_clone = payload_round2.clone();
    let post_handle =
        tokio::spawn(async move { post_data_to_server(http_port, payload_clone).await });
    post_handle.await??;
    sleep_1s().await;

    log::info!("🔍 Round 2: Polling data from slave");
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
            &register_length_arg,
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
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Round 2: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    log::info!("✅ Round 2: Received values: {:?}", response.values);
    if response.values != round2_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Round 2: Received values {:?} do not match expected {:?}",
            response.values,
            round2_values
        ));
    }

    // Test Round 3: Custom hex values
    log::info!("🔄 Round 3: Posting custom hex values to HTTP server");
    let payload_round3 = build_station_payload(&round3_values);
    let payload_clone = payload_round3.clone();
    let post_handle =
        tokio::spawn(async move { post_data_to_server(http_port, payload_clone).await });
    post_handle.await??;
    sleep_1s().await;

    log::info!("🔍 Round 3: Polling data from slave");
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
            &register_length_arg,
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
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Round 3: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    log::info!("✅ Round 3: Received values: {:?}", response.values);
    if response.values != round3_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Round 3: Received values {:?} do not match expected {:?}",
            response.values,
            round3_values
        ));
    }

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

    log::info!("✅ HTTP data source test passed (all 3 rounds verified)");
    Ok(())
}
