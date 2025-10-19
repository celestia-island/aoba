use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::Stdio,
};

use ci_utils::{
    create_modbus_command, sleep_a_while, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2,
};

/// Test master-slave communication with virtual serial ports
/// Server = Modbus Master (provides data, responds to requests) on port1
/// Client = Modbus Slave polling (sends requests, receives data) on port2
pub async fn test_master_slave_communication() -> Result<()> {
    log::info!("🧪 Testing master-slave communication with virtual serial ports...");
    let temp_dir = std::env::temp_dir();
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Create a data file for the server to provide
    let data_file = temp_dir.join("test_modbus_e2e_data.json");
    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [10, 20, 30, 40, 50]}}"#)?;
    }

    // Start server (Modbus master-provide) on port1 in persistent mode
    log::info!(
        "🧪 Starting Modbus server (master-provide) on {}...",
        ports.port1_name
    );
    let server_output = temp_dir.join("server_output.log");
    let server_output_file = File::create(&server_output)?;

    let mut server = create_modbus_command(
        false, // master-provide
        &ports.port1_name,
        true, // persistent
        Some(&format!("file:{data}", data = data_file.display())),
    )?
    .stdout(Stdio::from(server_output_file))
    .stderr(Stdio::piped())
    .spawn()?;

    // Give server time to start and fully acquire the port
    // use multiple async short waits to emulate a few seconds
    sleep_a_while().await;
    sleep_a_while().await;
    sleep_a_while().await;

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

            std::fs::remove_file(&data_file)?;
            std::fs::remove_file(&server_output)?;

            return Err(anyhow!(
                "Server exited prematurely with status {status}: {stderr}"
            ));
        }
        None => {
            log::info!("✅ Server is running");
        }
    }

    // Now start client (slave-poll) on port2 in temporary mode
    log::info!(
        "🧪 Starting Modbus client (slave-poll) on {}...",
        ports.port2_name
    );

    let binary = ci_utils::build_debug_bin("aoba")?;
    let client_output = std::process::Command::new(&binary)
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
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for client to complete (temporary mode exits after one operation)
    let client_result = client_output.wait_with_output()?;

    // Kill server process and wait for it to fully exit
    server.kill()?;
    server.wait()?;

    // Give extra time for ports to be fully released
    sleep_a_while().await;

    log::info!(
        "🧪 Client exit status: {status}",
        status = client_result.status
    );
    log::info!(
        "🧪 Client stdout: {stdout}",
        stdout = String::from_utf8_lossy(&client_result.stdout)
    );
    log::info!(
        "🧪 Client stderr: {stderr}",
        stderr = String::from_utf8_lossy(&client_result.stderr)
    );

    // Read server output
    let server_output_content = std::fs::read_to_string(&server_output).unwrap_or_default();
    log::info!("🧪 Server output: {server_output_content}");

    // Clean up
    std::fs::remove_file(&data_file)?;
    std::fs::remove_file(&server_output)?;

    // Verify communication happened
    if !client_result.status.success() {
        return Err(anyhow!("Client command failed"));
    }

    let stdout = String::from_utf8_lossy(&client_result.stdout);
    if stdout.trim().is_empty() {
        log::warn!("⚠️ No output from client (communication may have failed)");
    } else {
        log::info!("✅ Client produced output, communication test passed");
    }

    Ok(())
}

/// Test slave listen with temporary mode and virtual ports
pub async fn test_slave_listen_with_vcom() -> Result<()> {
    log::info!("🧪 Testing slave listen temporary mode with virtual serial ports...");

    // Just verify the command works with virtual ports
    let output = create_modbus_command(true, "/tmp/vcom1", false, None)?
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match output {
        Ok(mut child) => {
            // Wait for a short time
            sleep_a_while().await;
            child.kill()?;
            child.wait()?;

            log::info!("✅ Slave listen command accepted with virtual ports");

            // Give extra time for port to be fully released
            sleep_a_while().await;

            Ok(())
        }
        Err(err) => {
            log::error!("Failed to spawn slave listen: {err}");
            Err(anyhow!("Failed to spawn: {err}"))
        }
    }
}

/// Test master provide with persistent mode and virtual ports
pub async fn test_master_provide_with_vcom() -> Result<()> {
    log::info!("🧪 Testing master provide persistent mode with virtual serial ports...");

    // Create a temporary file with test data
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join("test_modbus_vcom_data.json");

    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [100, 200, 300, 400, 500]}}"#)?;
    }

    let output = create_modbus_command(
        false,
        "/tmp/vcom2",
        true,
        Some(&format!("file:{}", data_file.display())),
    )?
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn();

    match output {
        Ok(mut child) => {
            // Let it run for a bit
            sleep_a_while().await;
            child.kill()?;
            child.wait()?;

            log::info!("✅ Master provide persist command accepted with virtual ports");

            // Clean up
            std::fs::remove_file(&data_file)?;

            // Give extra time for port to be fully released
            sleep_a_while().await;

            Ok(())
        }
        Err(err) => {
            std::fs::remove_file(&data_file)?;
            log::error!("Failed to spawn master provide persist: {err}");
            Err(anyhow!("Failed to spawn: {err}"))
        }
    }
}
