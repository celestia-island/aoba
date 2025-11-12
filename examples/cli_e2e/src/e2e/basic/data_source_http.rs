use anyhow::Result;
use axum::{http::StatusCode, routing::get, serve, Router};
use reqwest::Client;
use std::process::Stdio;
use std::time::Duration;
use tokio::task;

use crate::utils::{build_debug_bin, sleep_1s, vcom_matchers_with_ports, DEFAULT_PORT1};

/// Start an axum server in a background task that responds with the provided payload.
async fn run_simple_server(payload: &'static str) -> Result<String> {
    let app = Router::new().route("/", get(move || async move { (StatusCode::OK, payload) }));

    // Bind a tokio TcpListener to an ephemeral port so we can discover the address
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // Serve the app in a background task. axum::serve runs forever, so spawn it.
    let server = serve(listener, app);
    task::spawn(async move {
        if let Err(e) = server.await {
            log::error!("server error: {}", e);
        }
    });

    // Wait for server to be reachable (use async reqwest to probe)
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

/// Test master mode with HTTP data source using axum for the test server
pub async fn test_http_data_source() -> Result<()> {
    log::info!("🧪 Testing HTTP data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, crate::utils::DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Prepare server payload and start server
    let payload = r#"{"values": [10, 20, 30, 40, 50]}"#;
    let server_url = run_simple_server(payload).await?;

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    // Start master with HTTP data source
    let server_output = temp_dir.join("server_http_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let mut master = std::process::Command::new(&binary)
        .args([
            "--master-provide",
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
            &server_url,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Give master time to fetch data and start
    sleep_1s().await;
    sleep_1s().await;

    // Check if master ran and exited successfully (one-shot mode)
    match master.wait()? {
        status if status.success() => {
            log::info!("✅ Master with HTTP data source completed successfully");
        }
        status => {
            std::fs::remove_file(&server_output).ok();
            return Err(anyhow::anyhow!(
                "Master exited with non-zero status: {}",
                status
            ));
        }
    }

    // Clean up
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ HTTP data source test passed");
    Ok(())
}

/// Test master mode with HTTP data source in persistent mode using axum
pub async fn test_http_data_source_persist() -> Result<()> {
    log::info!("🧪 Testing HTTP data source persistent mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, crate::utils::DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Prepare server payload and start server
    let payload = r#"{"values": [15, 25, 35, 45, 55]}"#;
    let server_url = run_simple_server(payload).await?;

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    // Start master with HTTP data source in persistent mode
    let server_output = temp_dir.join("server_http_persist_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let mut master = std::process::Command::new(&binary)
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
            &server_url,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Give master time to start and poll a few times
    sleep_1s().await;
    sleep_1s().await;
    sleep_1s().await;

    // Check if master is still running
    match master.try_wait()? {
        Some(status) => {
            std::fs::remove_file(&server_output).ok();
            return Err(anyhow::anyhow!(
                "Master exited prematurely with status {}",
                status
            ));
        }
        None => {
            log::info!("✅ Master with HTTP data source is running and polling");
        }
    }

    // Clean up
    master.kill().ok();
    let _ = master.wait();
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ HTTP data source persistent test passed");
    Ok(())
}
