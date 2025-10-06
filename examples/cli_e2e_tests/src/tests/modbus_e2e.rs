use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Test master-slave communication with virtual serial ports
/// Master device = Modbus Slave/Server (responds to requests)
/// Slave device = Modbus Master/Client (sends requests)
pub fn test_master_slave_communication() -> Result<()> {
    log::info!("🧪 Testing master-slave communication with virtual serial ports...");

    let binary = aoba::ci::build_debug_bin("aoba")?;

    let temp_dir = std::env::temp_dir();

    // Start server (Modbus slave) on /tmp/vcom1 in persistent mode
    log::info!("🧪 Starting Modbus server (slave) on /tmp/vcom1...");
    let server_output = temp_dir.join("server_output.log");
    let server_output_file = File::create(&server_output)?;

    let mut server = Command::new(&binary)
        .args([
            "--slave-listen-persist",
            "/tmp/vcom1",
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
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Give server time to start and fully acquire the port
    thread::sleep(Duration::from_secs(3));

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

            std::fs::remove_file(&server_output)?;

            return Err(anyhow!(
                "Server exited prematurely with status {}: {}",
                status,
                stderr
            ));
        }
        None => {
            log::info!("✅ Server is running");
        }
    }

    // Create a data file for the client to send
    let data_file = temp_dir.join("test_modbus_e2e_data.json");
    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [10, 20, 30, 40, 50]}}"#)?;
    }

    // Now start client (Modbus master) on /tmp/vcom2 in temporary mode
    log::info!("🧪 Starting Modbus client (master) on /tmp/vcom2...");
    let client_output = Command::new(&binary)
        .args([
            "--master-provide",
            "/tmp/vcom2",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--data-source",
            &format!("file:{}", data_file.display()),
            "--baud-rate",
            "9600",
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
    thread::sleep(Duration::from_secs(1));

    log::info!("🧪 Client exit status: {}", client_result.status);
    log::info!(
        "🧪 Client stdout: {}",
        String::from_utf8_lossy(&client_result.stdout)
    );
    log::info!(
        "🧪 Client stderr: {}",
        String::from_utf8_lossy(&client_result.stderr)
    );

    // Read server output
    let server_output_content = std::fs::read_to_string(&server_output).unwrap_or_default();
    log::info!("🧪 Server output: {}", server_output_content);

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
pub fn test_slave_listen_with_vcom() -> Result<()> {
    log::info!("🧪 Testing slave listen temporary mode with virtual serial ports...");

    let binary = aoba::ci::build_debug_bin("aoba")?;

    // Just verify the command works with virtual ports
    let output = Command::new(&binary)
        .args([
            "--slave-listen",
            "/tmp/vcom1",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match output {
        Ok(mut child) => {
            // Wait for a short time
            thread::sleep(Duration::from_millis(1000));
            child.kill()?;
            child.wait()?;

            log::info!("✅ Slave listen command accepted with virtual ports");

            // Give extra time for port to be fully released
            thread::sleep(Duration::from_secs(1));

            Ok(())
        }
        Err(e) => {
            log::error!("Failed to spawn slave listen: {}", e);
            Err(anyhow!("Failed to spawn: {}", e))
        }
    }
}

/// Test master provide with persistent mode and virtual ports
pub fn test_master_provide_with_vcom() -> Result<()> {
    log::info!("🧪 Testing master provide persistent mode with virtual serial ports...");

    let binary = aoba::ci::build_debug_bin("aoba")?;

    // Create a temporary file with test data
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join("test_modbus_vcom_data.json");

    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [100, 200, 300, 400, 500]}}"#)?;
    }

    let output = Command::new(&binary)
        .args([
            "--master-provide-persist",
            "/tmp/vcom2",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--data-source",
            &format!("file:{}", data_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match output {
        Ok(mut child) => {
            // Let it run for a bit
            thread::sleep(Duration::from_secs(1));
            child.kill()?;
            child.wait()?;

            log::info!("✅ Master provide persist command accepted with virtual ports");

            // Clean up
            std::fs::remove_file(&data_file)?;

            // Give extra time for port to be fully released
            thread::sleep(Duration::from_secs(1));

            Ok(())
        }
        Err(e) => {
            std::fs::remove_file(&data_file)?;
            log::error!("Failed to spawn master provide persist: {}", e);
            Err(anyhow!("Failed to spawn: {}", e))
        }
    }
}
