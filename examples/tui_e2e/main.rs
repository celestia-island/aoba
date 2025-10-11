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

    // Check for debug mode argument
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--debug".to_string()) || args.contains(&"debug".to_string()) {
        std::env::set_var("DEBUG_MODE", "1");
        log::info!("🔴 DEBUG MODE ENABLED - DebugBreakpoint actions will be active");
        log::info!("💡 Test will capture screen and exit at breakpoints");
    }

    // Check if we should loop the tests
    let loop_count = std::env::var("TEST_LOOP")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1);

    if loop_count > 1 {
        log::info!("🧪 Running tests in loop mode: {loop_count} iterations");
    }

    for iteration in 1..=loop_count {
        if loop_count > 1 {
            log::info!("🧪 ===== Iteration {iteration}/{loop_count} =====");
        }

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
            break;
        }

        log::info!("🧪 Virtual serial ports available, running E2E tests...");

        // Test 0: CLI port release test - verify CLI properly releases ports on exit
        log::info!("🧪 Test 0/2: CLI port release verification");
        tests::test_cli_port_release().await?;

        // Reset ports after CLI cleanup test to remove any lingering locks from the spawned CLI process
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 0...");
            setup_virtual_serial_ports()?;
        }

        // Test 1: TUI Master-Provide + CLI Slave-Poll with 10 rounds of continuous random data
        log::info!(
            "🧪 Test 1/2: TUI Master-Provide + CLI Slave-Poll (10 rounds, holding registers)"
        );
        tests::test_tui_slave_with_cli_master_continuous().await?;

        // Reset ports after test completes (Unix only)
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 1...");
            setup_virtual_serial_ports()?;
        }

        // Test 2: TUI Master-Provide + CLI Slave-Poll (repeat for stability)
        log::info!("🧪 Test 2/2: TUI Master-Provide + CLI Slave-Poll - Repeat (10 rounds, holding registers)");
        tests::test_tui_master_with_cli_slave_continuous().await?;

        // Reset ports after test completes (Unix only)
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 2...");
            setup_virtual_serial_ports()?;
        }

        if loop_count > 1 {
            log::info!("✅ Iteration {iteration}/{loop_count} completed successfully!");
        } else {
            log::info!("🧪 All TUI E2E tests passed!");
        }
    }

    if loop_count > 1 {
        log::info!("🎉 All {loop_count} iterations completed successfully!");
    }

    Ok(())
}
