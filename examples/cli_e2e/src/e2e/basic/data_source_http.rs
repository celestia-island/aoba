use anyhow::{anyhow, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use reqwest::Client;
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::task;

use axum::{http::StatusCode, routing::get, serve, Router};

use crate::utils::{
    build_debug_bin, sleep_1s, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2,
};
use aoba::protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode};

async fn run_simple_server(payload: Arc<String>) -> Result<String> {
    let response_body = payload.clone();
    let app = Router::new().route(
        "/",
        get(move || {
            let body = response_body.clone();
            async move { (StatusCode::OK, body.as_ref().clone()) }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let server = serve(listener, app);
    task::spawn(async move {
        if let Err(e) = server.await {
            log::error!("server error: {e}");
        }
    });

    let url = format!("http://{}", addr);
    let client = Client::new();
    let mut attempts = 0;
    while attempts < 20 {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => break,
            _ => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                attempts += 1;
            }
        }
    }

    Ok(url)
}

fn build_station_payload(values: &[u16]) -> Result<Arc<String>> {
    let stations = vec![StationConfig::single_range(
        1,
        StationMode::Master,
        RegisterMode::Holding,
        0,
        10,
        Some(values.to_vec()),
    )];
    let json = serde_json::to_string(&stations)?;
    Ok(Arc::new(json))
}

fn parse_client_values(stdout: &[u8]) -> Result<Vec<u16>> {
    let output = String::from_utf8_lossy(stdout);
    let response_line = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow!("Client produced empty stdout"))?;
    let json: serde_json::Value = serde_json::from_str(response_line)?;
    let values = json
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("Response missing values array"))?;
    Ok(values
        .iter()
        .map(|v| v.as_u64().unwrap_or(0) as u16)
        .collect())
}

/// Test master mode with HTTP data source using axum for the test server
pub async fn test_http_data_source() -> Result<()> {
    log::info!("🧪 Testing HTTP data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0xA0BA_DA7A_01_u64);
    let expected_values: Vec<u16> = (0..10).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values)?;
    let server_url = run_simple_server(payload).await?;

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    let server_output = temp_dir.join("server_http_output.log");
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
            &server_url,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    sleep_1s().await;
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
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
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

    let received_values = parse_client_values(&client_output.stdout)?;
    if received_values != expected_values {
        return Err(anyhow!(
            "Received values {:?} do not match expected {:?}",
            received_values,
            expected_values
        ));
    }

    log::info!("✅ HTTP data source test passed");
    Ok(())
}

/// Test master mode with HTTP data source in persistent mode using axum
pub async fn test_http_data_source_persist() -> Result<()> {
    log::info!("🧪 Testing HTTP data source persistent mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0xA0BA_DA7A_02_u64);
    let expected_values: Vec<u16> = (0..10).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values)?;
    let server_url = run_simple_server(payload).await?;

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    let server_output = temp_dir.join("server_http_persist_output.log");
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
            &server_url,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    sleep_1s().await;
    sleep_1s().await;
    sleep_1s().await;

    if let Some(status) = master.try_wait()? {
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!("Master exited prematurely with status {status}"));
    }

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

    let received_values = parse_client_values(&client_output.stdout)?;
    if received_values != expected_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Received values {:?} do not match expected {:?}",
            received_values,
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
