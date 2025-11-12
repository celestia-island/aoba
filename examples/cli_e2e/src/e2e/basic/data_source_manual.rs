use anyhow::Result;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::Stdio,
};

use crate::utils::{
    build_debug_bin, sleep_1s, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2,
};

/// Test master mode with manual data source
/// This tests that the manual data source mode allows the master to start without external data
pub async fn test_manual_data_source() -> Result<()> {
    log::info!("🧪 Testing manual data source mode...");
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
    sleep_1s().await;
    sleep_1s().await;

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
/// This tests that the IPC pipe data source mode can be configured
pub async fn test_ipc_pipe_data_source() -> Result<()> {
    log::info!("🧪 Testing IPC pipe data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Create IPC pipe path - use a regular file for testing since mkfifo needs nix crate
    let ipc_pipe = temp_dir.join("test_ipc_pipe_file");

    // Create a test file with initial data
    {
        let mut file = File::create(&ipc_pipe)?;
        writeln!(file, r#"{{"values": [100, 200, 300, 400, 500]}}"#)?;
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

    // Give server time to start
    sleep_1s().await;
    sleep_1s().await;

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

    // Clean up
    server.kill().ok();
    let _ = server.wait();
    std::fs::remove_file(&ipc_pipe).ok();
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ IPC pipe data source test passed");
    Ok(())
}
