mod basic_master;
mod basic_slave;
mod cli_port_cleanup;
mod e2e;
mod utils;

use anyhow::Result;
#[cfg(not(windows))]
use std::process::Command;

use basic_master::test_tui_master_with_cli_slave_continuous;
use basic_slave::test_tui_slave_with_cli_master_continuous;
use cli_port_cleanup::test_cli_port_release;

/// Setup virtual serial ports by running socat_init script without requiring sudo
/// This function can be called before each test to reset ports
pub fn setup_virtual_serial_ports() -> Result<bool> {
    #[cfg(windows)]
    {
        log::info!("🧪 Windows platform: skipping virtual serial port setup (socat not available)");
        return Ok(false);
    }

    #[cfg(not(windows))]
    {
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

        // Run the script directly; it operates entirely in user-mode
        let output = Command::new("bash")
            .arg(script_path)
            .arg("--mode")
            .arg("tui")
            .output()?;

        if output.status.success() {
            apply_port_env_overrides(&output.stdout);
            log::info!("✅ Virtual serial ports reset successfully");
            Ok(true)
        } else {
            log::warn!("⚠️ Failed to setup virtual serial ports:");
            log::warn!(
                "stdout: {stdout}",
                stdout = String::from_utf8_lossy(&output.stdout)
            );
            log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            Ok(false)
        }
    }
}

#[cfg(not(windows))]
fn apply_port_env_overrides(stdout: &[u8]) {
    let mut port1: Option<String> = None;
    let mut port2: Option<String> = None;

    for line in String::from_utf8_lossy(stdout).lines() {
        if let Some(value) = line.strip_prefix("PORT1=") {
            port1 = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("PORT2=") {
            port2 = Some(value.trim().to_string());
        }
    }

    if let Some(p1) = port1 {
        std::env::set_var("AOBATEST_PORT1", &p1);
        log::info!("🔗 Using virtual port override: AOBATEST_PORT1={p1}");
    }
    if let Some(p2) = port2 {
        std::env::set_var("AOBATEST_PORT2", &p2);
        log::info!("🔗 Using virtual port override: AOBATEST_PORT2={p2}");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("🧪 Starting TUI E2E Tests...");

    // On Unix-like systems, try to setup virtual serial ports
    #[cfg(not(windows))]
    {
        log::info!("🧪 Setting up virtual serial ports...");
        setup_virtual_serial_ports()?;
    }

    // Check if we should run virtual serial port tests
    if !ci_utils::should_run_vcom_tests() {
        log::warn!("⚠️ Virtual serial ports not available, skipping E2E tests");
        return Ok(());
    }

    log::info!("🧪 Virtual serial ports available, running E2E tests...");

    // Test 0: CLI port release test - verify CLI properly releases ports on exit
    log::info!("🧪 Test 0/4: CLI port release verification");
    test_cli_port_release().await?;

    // Reset ports after CLI cleanup test to remove any lingering locks from the spawned CLI process
    #[cfg(not(windows))]
    {
        log::info!("🧪 Resetting virtual serial ports after Test 0...");
        setup_virtual_serial_ports()?;
    }

    // Test 1: TUI Slave + CLI Master with 10 rounds of continuous random data
    log::info!("🧪 Test 1/4: TUI Slave + CLI Master (10 rounds, holding registers)");
    test_tui_slave_with_cli_master_continuous().await?;

    // Reset ports after test completes (Unix only)
    #[cfg(not(windows))]
    {
        log::info!("🧪 Resetting virtual serial ports after Test 1...");
        setup_virtual_serial_ports()?;
    }

    // Test 2: TUI Master + CLI Slave (repeat for stability)
    log::info!("🧪 Test 2/4: TUI Master + CLI Slave - Repeat (10 rounds, holding registers)");
    test_tui_master_with_cli_slave_continuous().await?;

    // Reset ports after test completes (Unix only)
    #[cfg(not(windows))]
    {
        log::info!("🧪 Resetting virtual serial ports after Test 2...");
        setup_virtual_serial_ports()?;
    }

    // Test 3: Multiple TUI Masters on vcom1
    log::info!("🧪 Test 3/4: Multiple TUI Masters on vcom1 (E2E test suite)");
    e2e::test_tui_multi_masters().await?;

    // Reset ports after test completes (Unix only)
    #[cfg(not(windows))]
    {
        log::info!("🧪 Resetting virtual serial ports after Test 3...");
        setup_virtual_serial_ports()?;
    }

    // Test 4: Multiple TUI Slaves on vcom2
    log::info!("🧪 Test 4/4: Multiple TUI Slaves on vcom2 (E2E test suite)");
    e2e::test_tui_multi_slaves().await?;

    log::info!("🧪 All TUI E2E tests passed!");

    Ok(())
}
