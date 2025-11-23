/// Test API Master Example with CLI Slave
///
/// This test verifies that the API master example can communicate with a CLI slave.
/// The API master uses the trait-based API with a fixed data source, while the CLI slave
/// listens and responds to requests.
use anyhow::{anyhow, Result};
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use crate::utils::{build_debug_bin, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2};
use _main::utils::sleep_3s;

pub async fn test_api_master_with_cli_slave() -> Result<()> {
    log::info!("🧪 Testing API Master with CLI Slave communication...");
    
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Start CLI slave on port2
    log::info!("🧪 Starting CLI Slave on {}...", ports.port2_name);
    let aoba_binary = build_debug_bin("aoba")?;
    
    let mut cli_slave = Command::new(&aoba_binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-listen-persist",
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
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give CLI slave time to start and bind to port
    sleep_3s().await;

    // Check if CLI slave is still running
    match cli_slave.try_wait()? {
        Some(status) => {
            return Err(anyhow!(
                "CLI Slave exited prematurely with status {}",
                status
            ));
        }
        None => {
            log::info!("✅ CLI Slave is running");
        }
    }

    // Start API master on port1
    log::info!("🧪 Starting API Master on {}...", ports.port1_name);
    let api_master_binary = build_debug_bin("api_master")?;
    
    let mut api_master = Command::new(&api_master_binary)
        .arg(&ports.port1_name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for API master to complete (it runs for 10 iterations)
    // The API master example is designed to exit after 10 successful responses
    log::info!("⏳ Waiting for API Master to complete...");
    
    let timeout_duration = Duration::from_secs(30);
    let start_time = std::time::Instant::now();
    
    loop {
        match api_master.try_wait()? {
            Some(status) => {
                if status.success() {
                    log::info!("✅ API Master completed successfully");
                    break;
                } else {
                    let stderr = {
                        let mut buf = Vec::new();
                        if let Some(mut stderr) = api_master.stderr.take() {
                            use std::io::Read;
                            stderr.read_to_end(&mut buf)?;
                        }
                        String::from_utf8_lossy(&buf).to_string()
                    };
                    
                    // Clean up
                    cli_slave.kill()?;
                    
                    return Err(anyhow!(
                        "API Master failed with status {}: {}",
                        status,
                        stderr
                    ));
                }
            }
            None => {
                if start_time.elapsed() > timeout_duration {
                    // Timeout - kill both processes
                    api_master.kill()?;
                    cli_slave.kill()?;
                    return Err(anyhow!("API Master test timed out after 30 seconds"));
                }
                // Still running, wait a bit
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    // Verify CLI slave output
    cli_slave.kill()?;
    let cli_output = cli_slave.wait_with_output()?;
    let cli_stdout = String::from_utf8_lossy(&cli_output.stdout);
    
    log::info!("📋 CLI Slave output sample:");
    for line in cli_stdout.lines().take(5) {
        log::info!("  {}", line);
    }

    // Verify API master output
    let api_output = api_master.wait_with_output()?;
    let api_stdout = String::from_utf8_lossy(&api_output.stdout);
    
    log::info!("📋 API Master output sample:");
    for line in api_stdout.lines().take(10) {
        log::info!("  {}", line);
    }

    // Basic validation - check that we got some responses
    let response_count = api_stdout.matches("Response #").count();
    log::info!("📊 API Master received {} responses", response_count);
    
    if response_count < 5 {
        return Err(anyhow!(
            "Expected at least 5 responses, got {}",
            response_count
        ));
    }

    log::info!("✅ API Master + CLI Slave test passed!");
    Ok(())
}
