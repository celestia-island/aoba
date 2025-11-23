/// Test API Slave Example with CLI Master
///
/// This test verifies that the API slave example can respond to a CLI master.
/// The API slave uses the trait-based API to handle requests, while the CLI master
/// polls and reads data.
use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};

use crate::utils::{build_debug_bin, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2};
use _main::utils::{sleep_1s, sleep_3s};

pub async fn test_api_slave_with_cli_master() -> Result<()> {
    log::info!("🧪 Testing API Slave with CLI Client (slave-poll) communication...");
    
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

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

    // Start CLI client (slave-poll) on port2 to poll the API Slave
    log::info!("🧪 Starting CLI Client (slave-poll) on {}...", ports.port2_name);
    let aoba_binary = build_debug_bin("aoba")?;
    
    // Run CLI client with slave-poll (it will poll once and exit in temporary mode)
    let cli_output = Command::new(&aoba_binary)
        .arg("--enable-virtual-ports")
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
        .output()?;

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

    // Stop API slave and get output (logs go to stderr)
    api_slave.kill()?;
    let api_output = api_slave.wait_with_output()?;
    let api_stderr = String::from_utf8_lossy(&api_output.stderr);
    
    log::info!("📋 API Slave output sample:");
    for line in api_stderr.lines().take(10) {
        log::info!("  {}", line);
    }

    // Basic validation - check that API slave processed at least 1 request
    // Note: CLI client (--slave-poll) in temporary mode only polls once by default
    let request_count = api_stderr.matches("Request #").count();
    log::info!("📊 API Slave processed {} requests", request_count);
    
    if request_count < 1 {
        return Err(anyhow!(
            "Expected at least 1 request processed, got {}",
            request_count
        ));
    }

    // Validate CLI client got responses
    let response_count = cli_stdout.lines().filter(|l| l.contains("values")).count();
    log::info!("📊 CLI Client received {} JSON responses", response_count);
    
    if response_count < 1 {
        return Err(anyhow!(
            "Expected at least 1 JSON response, got {}",
            response_count
        ));
    }

    log::info!("✅ API Slave + CLI Master test passed!");
    Ok(())
}
