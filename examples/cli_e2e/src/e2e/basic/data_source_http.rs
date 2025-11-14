use anyhow::{anyhow, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{process::Stdio, sync::Arc};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use aoba::{
    cli::modbus::ModbusResponse,
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

/// Test master mode with HTTP data source - master runs HTTP server, test POSTs data
pub async fn test_http_data_source() -> Result<()> {
    log::info!("🧪 Testing HTTP data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0x00A0_BADA_7A01_u64);
    let expected_values: Vec<u16> = (0..REGISTER_LENGTH).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values);

    // Use a fixed port for the HTTP server that the master will run
    let http_port = HTTP_SERVER_PORT_DEFAULT;
    let data_source_arg = format!("http://{}", http_port);

    log::info!(
        "🧪 Master will run HTTP server on port {}",
        HTTP_SERVER_PORT_DEFAULT
    );

    let server_output = temp_dir.join("server_http_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

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
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &data_source_arg,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for master to be ready with flexible waiting (minimum 3 seconds)
    wait_for_process_ready(&mut master, 3000).await?;

    // POST data to the master's HTTP server in a separate thread
    let payload_clone = payload.clone();
    let post_handle =
        tokio::spawn(async move { post_data_to_server(http_port, payload_clone).await });

    // Wait for POST to complete
    post_handle.await??;

    // Give the master time to process the HTTP data
    sleep_1s().await;

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

    log::info!("✅ HTTP data source test passed");
    Ok(())
}

/// Test master mode with HTTP data source in persistent mode - master runs HTTP server, test POSTs data
pub async fn test_http_data_source_persist() -> Result<()> {
    log::info!("🧪 Testing HTTP data source persistent mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0x00A0_BADA_7A02_u64);
    let expected_values: Vec<u16> = (0..REGISTER_LENGTH).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values);

    // Use a fixed port for the HTTP server that the master will run
    let http_port = HTTP_SERVER_PORT_DEFAULT;
    let data_source_arg = format!("http://{}", http_port);

    log::info!(
        "🧪 Master will run HTTP server on port {}",
        HTTP_SERVER_PORT_DEFAULT
    );

    let server_output = temp_dir.join("server_http_persist_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

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
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for master to be ready with flexible waiting (minimum 3 seconds)
    wait_for_process_ready(&mut master, 3000).await?;

    // POST data to the master's HTTP server in a separate thread
    let payload_clone = payload.clone();
    let post_handle =
        tokio::spawn(async move { post_data_to_server(http_port, payload_clone).await });

    // Wait for POST to complete
    post_handle.await??;

    // Give the master time to process the HTTP data
    sleep_1s().await;

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

    log::info!("✅ HTTP data source persistent test passed");
    Ok(())
}
