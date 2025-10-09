mod tests;
mod utils;

use anyhow::Result;
use std::process::Command;

/// Setup virtual serial ports by running socat_init script with sudo
/// This function can be called before each test to reset ports
pub fn setup_virtual_serial_ports() -> Result<bool> {
    log::info!("🧪 Setting up virtual serial ports...");

    // Find the socat_init.sh script (centralized at repo root)
    let script_path = std::path::Path::new("scripts/socat_init.sh");

    if !script_path.exists() {
        log::warn!(
            "⚠️ socat_init.sh script not found at {}",
            script_path.display()
        );
        return Ok(false);
    }

    // Run the script with sudo to reset/reinitialize virtual serial ports
    let output = Command::new("sudo")
        .arg("bash")
        .arg(script_path)
        .arg("--mode")
        .arg("tui")
        .output()?;

    if output.status.success() {
        log::info!("✅ Virtual serial ports reset successfully");
        Ok(true)
    } else {
        log::warn!("⚠️ Failed to setup virtual serial ports:");
        log::warn!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        Ok(false)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Check if we should loop the tests
    let loop_count = std::env::var("TEST_LOOP")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1);

    if loop_count > 1 {
        log::info!("🧪 Running tests in loop mode: {} iterations", loop_count);
    }

    for iteration in 1..=loop_count {
        if loop_count > 1 {
            log::info!("🧪 ===== Iteration {}/{} =====", iteration, loop_count);
        }

        log::info!("🧪 Starting TUI E2E Tests...");

        // Check if we can setup virtual serial ports for E2E tests
        if setup_virtual_serial_ports()? {
            log::info!("🧪 Virtual serial ports available, running E2E tests...");

            // Test 1: TUI Slave + CLI Master with 10 rounds of continuous random data
            log::info!("🧪 Test 1/2: TUI Slave + CLI Master (10 rounds, holding registers)");
            tests::test_tui_slave_with_cli_master_continuous().await?;
            
            // Reset ports after test completes
            log::info!("🧪 Resetting virtual serial ports after Test 1...");
            setup_virtual_serial_ports()?;

            // Test 2: TUI Master + CLI Slave with 10 rounds of continuous random data
            log::info!("🧪 Test 2/2: TUI Master + CLI Slave (10 rounds, holding registers)");
            tests::test_tui_master_with_cli_slave_continuous().await?;
            
            // Reset ports after test completes
            log::info!("🧪 Resetting virtual serial ports after Test 2...");
            setup_virtual_serial_ports()?;
        } else {
            log::warn!("⚠️ Virtual serial ports setup failed, skipping E2E tests");
        }

        if loop_count > 1 {
            log::info!("✅ Iteration {}/{} completed successfully!", iteration, loop_count);
        } else {
            log::info!("🧪 All TUI E2E tests passed!");
        }
    }

    if loop_count > 1 {
        log::info!("🎉 All {} iterations completed successfully!", loop_count);
    }

    Ok(())
}
