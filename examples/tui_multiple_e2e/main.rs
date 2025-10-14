mod tests;

use anyhow::Result;
use std::process::Command;

/// Setup virtual serial ports by running socat_init script with tui_multiple mode
pub fn setup_virtual_serial_ports() -> Result<bool> {
    log::info!("🧪 Setting up 6 virtual serial ports for multiple E2E test...");

    // Find the socat_init.sh script (centralized at repo root)
    let script_path = std::path::Path::new("scripts/socat_init.sh");

    if !script_path.exists() {
        log::warn!(
            "⚠️ socat_init.sh script not found at {}",
            script_path.display()
        );
        return Ok(false);
    }

    // Run the script with tui_multiple mode
    let output = Command::new("bash")
        .arg(script_path)
        .arg("--mode")
        .arg("tui_multiple")
        .output()?;

    if output.status.success() {
        apply_port_env_overrides(&output.stdout);
        log::info!("✅ Virtual serial ports (6 ports) reset successfully");
        Ok(true)
    } else {
        log::warn!("⚠️ Failed to setup virtual serial ports:");
        log::warn!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        Ok(false)
    }
}

fn apply_port_env_overrides(stdout: &[u8]) {
    let mut ports: Vec<(String, String)> = Vec::new();

    for line in String::from_utf8_lossy(stdout).lines() {
        if let Some(value) = line.strip_prefix("PORT1=") {
            ports.push(("AOBATEST_PORT1".to_string(), value.trim().to_string()));
        } else if let Some(value) = line.strip_prefix("PORT2=") {
            ports.push(("AOBATEST_PORT2".to_string(), value.trim().to_string()));
        } else if let Some(value) = line.strip_prefix("PORT3=") {
            ports.push(("AOBATEST_PORT3".to_string(), value.trim().to_string()));
        } else if let Some(value) = line.strip_prefix("PORT4=") {
            ports.push(("AOBATEST_PORT4".to_string(), value.trim().to_string()));
        } else if let Some(value) = line.strip_prefix("PORT5=") {
            ports.push(("AOBATEST_PORT5".to_string(), value.trim().to_string()));
        } else if let Some(value) = line.strip_prefix("PORT6=") {
            ports.push(("AOBATEST_PORT6".to_string(), value.trim().to_string()));
        }
    }

    for (env_var, value) in ports {
        std::env::set_var(&env_var, &value);
        log::info!("🔗 Using virtual port override: {env_var}={value}");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("🧪 Starting TUI Multiple E2E Tests...");

    // On Unix-like systems, try to setup virtual serial ports
    #[cfg(not(windows))]
    {
        log::info!("🧪 Setting up 6 virtual serial ports...");
        setup_virtual_serial_ports()?;
    }

    // Check if we should run virtual serial port tests
    if !ci_utils::should_run_vcom_tests() {
        log::warn!("⚠️ Virtual serial ports not available, skipping E2E tests");
        return Ok(());
    }

    log::info!("🧪 Virtual serial ports available, running Multiple E2E tests...");

    // Test: Multiple Masters and Slaves test
    log::info!("🧪 Test: Multiple Masters and Slaves with interference handling");
    tests::test_multiple_masters_slaves().await?;

    // Reset ports after test completes (Unix only)
    #[cfg(not(windows))]
    {
        log::info!("🧪 Resetting virtual serial ports after test...");
        setup_virtual_serial_ports()?;
    }

    log::info!("🎉 All TUI Multiple E2E tests passed!");

    Ok(())
}
