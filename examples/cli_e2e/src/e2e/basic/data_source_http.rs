use anyhow::Result;
use std::{io::Write, net::TcpListener, process::Stdio, thread, time::Duration};

use crate::utils::{build_debug_bin, sleep_1s, vcom_matchers_with_ports, DEFAULT_PORT1};

/// Test master mode with HTTP data source
/// This test starts a simple HTTP server and verifies the master can fetch data from it
pub async fn test_http_data_source() -> Result<()> {
    log::info!("🧪 Testing HTTP data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, crate::utils::DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Start a simple HTTP server on a random port
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let server_addr = listener.local_addr()?;
    let server_url = format!("http://{}", server_addr);

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    // Spawn HTTP server thread
    let server_handle = thread::spawn(move || {
        // Accept one connection and respond with JSON data
        if let Ok((mut stream, _)) = listener.accept() {
            let response = r#"HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 31

{"values": [10, 20, 30, 40, 50]}"#;
            let _ = stream.write_all(response.as_bytes());
        }
    });

    // Give server time to start
    std::thread::sleep(Duration::from_millis(100));

    // Start master with HTTP data source
    log::info!(
        "🧪 Starting Modbus master with HTTP data source on {}...",
        ports.port1_name
    );
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

    // Wait for server thread
    let _ = server_handle.join();

    // Clean up
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ HTTP data source test passed");
    Ok(())
}

/// Test master mode with HTTP data source in persistent mode
/// This verifies the master can continuously poll an HTTP endpoint
pub async fn test_http_data_source_persist() -> Result<()> {
    log::info!("🧪 Testing HTTP data source persistent mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, crate::utils::DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Start a simple HTTP server on a random port
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let server_addr = listener.local_addr()?;
    let server_url = format!("http://{}", server_addr);

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    // Spawn HTTP server thread that handles multiple requests
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running_clone = running.clone();

    let server_handle = thread::spawn(move || {
        while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok((mut stream, _)) = listener.accept() {
                let response = r#"HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 31

{"values": [15, 25, 35, 45, 55]}"#;
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });

    // Give server time to start
    std::thread::sleep(Duration::from_millis(100));

    // Start master with HTTP data source in persistent mode
    log::info!(
        "🧪 Starting Modbus master (persistent) with HTTP data source on {}...",
        ports.port1_name
    );
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
            running.store(false, std::sync::atomic::Ordering::Relaxed);
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
    running.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = server_handle.join();
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ HTTP data source persistent test passed");
    Ok(())
}
