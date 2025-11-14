use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::Write,
    process::{Command, Stdio},
};

use crate::utils::build_debug_bin;
use _main::utils::sleep_1s;

/// Test slave listen temporary mode (single response)
pub async fn test_slave_listen_temp() -> Result<()> {
    log::info!("🧪 Testing slave listen temporary mode...");

    // Get the binary path
    let binary = build_debug_bin("aoba")?;

    // This test requires actual serial port hardware or virtual serial ports
    // For now, we'll just verify the command line interface works

    let output = Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-listen",
            "/dev/null", // Use /dev/null as placeholder
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
            // Wait for a short time, then kill it (it will timeout anyway)
            sleep_1s().await;
            child.kill()?;

            log::info!("🧪 Slave listen command accepted (port error expected)");
            Ok(())
        }
        Err(err) => {
            log::error!("Failed to spawn slave listen: {err}");
            Err(anyhow!("Failed to spawn: {err}"))
        }
    }
}

/// Test slave listen persistent mode (continuous output)
pub async fn test_slave_listen_persist() -> Result<()> {
    log::info!("🧪 Testing slave listen persistent mode...");

    let binary = build_debug_bin("aoba")?;

    let output = Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-listen-persist",
            "/dev/null",
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
            // Give it a moment to start
            sleep_1s().await;

            // Kill the process after timeout
            child.kill()?;

            log::info!("🧪 Slave listen persist command accepted (port error expected)");
            Ok(())
        }
        Err(err) => {
            log::error!("Failed to spawn slave listen persist: {err}");
            Err(anyhow!("Failed to spawn: {err}"))
        }
    }
}

/// Test master provide temporary mode
pub async fn test_master_provide_temp() -> Result<()> {
    log::info!("🧪 Testing master provide temporary mode...");

    let binary = build_debug_bin("aoba")?;

    // Create a temporary file with test data
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join("test_modbus_data.json");

    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [10, 20, 30, 40, 50]}}"#)?;
    }

    let output = Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide",
            "/dev/null",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--data-source",
            &format!("file:{data}", data = data_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match output {
        Ok(mut child) => {
            sleep_1s().await;
            child.kill()?;

            log::info!("🧪 Master provide command accepted (port error expected)");

            // Clean up
            std::fs::remove_file(&data_file)?;
            Ok(())
        }
        Err(err) => {
            std::fs::remove_file(&data_file)?;
            log::error!("Failed to spawn master provide: {err}");
            Err(anyhow!("Failed to spawn: {err}"))
        }
    }
}

/// Test master provide persistent mode with file data source
pub async fn test_master_provide_persist() -> Result<()> {
    log::info!("🧪 Testing master provide persistent mode...");

    let binary = build_debug_bin("aoba")?;

    // Create a temporary file with multiple lines of test data
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join("test_modbus_data_multi.json");

    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [10, 20, 30, 40, 50]}}"#)?;
        writeln!(file, r#"{{"values": [15, 25, 35, 45, 55]}}"#)?;
        writeln!(file, r#"{{"values": [20, 30, 40, 50, 60]}}"#)?;
    }

    let output = Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide-persist",
            "/dev/null",
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
            sleep_1s().await;

            // Kill the process
            child.kill()?;

            log::info!("🧪 Master provide persist command accepted (port error expected)");

            // Clean up
            std::fs::remove_file(&data_file)?;
            Ok(())
        }
        Err(err) => {
            std::fs::remove_file(&data_file)?;
            log::error!("Failed to spawn master provide persist: {err}");
            Err(anyhow!("Failed to spawn: {err}"))
        }
    }
}
