/// Test API Slave Example with CLI Master
///
/// This test verifies that the API slave example can respond to a CLI master.
/// The API slave uses the trait-based API to handle requests, while the CLI master
/// polls and reads data.
use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::Write,
    process::{Command, Stdio},
};

use crate::utils::{build_debug_bin, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2};
use _main::utils::{sleep_1s, sleep_3s};

pub async fn test_api_slave_with_cli_master() -> Result<()> {
    log::info!("🧪 Testing API Slave with CLI Master communication...");
    
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Start API slave on port1
    log::info!("🧪 Starting API Slave on {}...", ports.port1_name);
    let api_slave_binary = build_debug_bin("api_slave")?;
    
    let mut api_slave = Command::new(&api_slave_binary)
        .arg(&ports.port1_name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give API slave time to start and bind to port
    sleep_3s().await;

    // Check if API slave is still running
    match api_slave.try_wait()? {
        Some(status) => {
            return Err(anyhow!(
                "API Slave exited prematurely with status {}",
                status
            ));
        }
        None => {
            log::info!("✅ API Slave is running");
        }
    }

    // Create a data file for CLI master to provide
    let data_file = temp_dir.join("test_api_slave_cli_master_data.json");
    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [10, 20, 30, 40, 50]}}"#)?;
    }

    // Start CLI master on port2 in temporary mode (it will exit after a few polls)
    log::info!("🧪 Starting CLI Master on {}...", ports.port2_name);
    let aoba_binary = build_debug_bin("aoba")?;
    
    // Run CLI master for a short duration (5 iterations)
    let cli_output = Command::new(&aoba_binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-poll",
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
            "--data-source",
            &format!("file:{}", data_file.display()),
            "--poll-count",
            "5",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    // Clean up data file
    std::fs::remove_file(&data_file)?;

    if !cli_output.status.success() {
        let stderr = String::from_utf8_lossy(&cli_output.stderr);
        api_slave.kill()?;
        return Err(anyhow!("CLI Master failed: {}", stderr));
    }

    let cli_stdout = String::from_utf8_lossy(&cli_output.stdout);
    log::info!("📋 CLI Master output sample:");
    for line in cli_stdout.lines().take(10) {
        log::info!("  {}", line);
    }

    // Wait a moment for API slave to process all requests
    sleep_1s().await;

    // Stop API slave and get output
    api_slave.kill()?;
    let api_output = api_slave.wait_with_output()?;
    let api_stdout = String::from_utf8_lossy(&api_output.stdout);
    
    log::info!("📋 API Slave output sample:");
    for line in api_stdout.lines().take(10) {
        log::info!("  {}", line);
    }

    // Basic validation - check that API slave processed some requests
    let request_count = api_stdout.matches("Request #").count();
    log::info!("📊 API Slave processed {} requests", request_count);
    
    if request_count < 3 {
        return Err(anyhow!(
            "Expected at least 3 requests processed, got {}",
            request_count
        ));
    }

    // Validate CLI master got responses
    let response_count = cli_stdout.lines().filter(|l| l.contains("values")).count();
    log::info!("📊 CLI Master received {} JSON responses", response_count);
    
    if response_count < 3 {
        return Err(anyhow!(
            "Expected at least 3 JSON responses, got {}",
            response_count
        ));
    }

    log::info!("✅ API Slave + CLI Master test passed!");
    Ok(())
}
