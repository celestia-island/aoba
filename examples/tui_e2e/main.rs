mod tests;
mod utils;

use anyhow::Result;
use std::process::Command;

/// Setup virtual serial ports by running socat_init script without requiring sudo
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
        log::warn!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        Ok(false)
    }
}

/// Setup 6 virtual serial ports for multiple master/slave test
pub fn setup_multiple_virtual_serial_ports() -> Result<bool> {
    log::info!("🧪 Setting up 6 virtual serial ports for multiple test...");

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
        log::info!("✅ 6 virtual serial ports setup successfully");
        Ok(true)
    } else {
        log::warn!("⚠️ Failed to setup 6 virtual serial ports:");
        log::warn!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        Ok(false)
    }
}

fn apply_port_env_overrides(stdout: &[u8]) {
    let mut port1: Option<String> = None;
    let mut port2: Option<String> = None;
    let mut port3: Option<String> = None;
    let mut port4: Option<String> = None;
    let mut port5: Option<String> = None;
    let mut port6: Option<String> = None;

    for line in String::from_utf8_lossy(stdout).lines() {
        if let Some(value) = line.strip_prefix("PORT1=") {
            port1 = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("PORT2=") {
            port2 = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("PORT3=") {
            port3 = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("PORT4=") {
            port4 = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("PORT5=") {
            port5 = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("PORT6=") {
            port6 = Some(value.trim().to_string());
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
    if let Some(p3) = port3 {
        std::env::set_var("AOBATEST_PORT3", &p3);
        log::info!("🔗 Using virtual port override: AOBATEST_PORT3={p3}");
    }
    if let Some(p4) = port4 {
        std::env::set_var("AOBATEST_PORT4", &p4);
        log::info!("🔗 Using virtual port override: AOBATEST_PORT4={p4}");
    }
    if let Some(p5) = port5 {
        std::env::set_var("AOBATEST_PORT5", &p5);
        log::info!("🔗 Using virtual port override: AOBATEST_PORT5={p5}");
    }
    if let Some(p6) = port6 {
        std::env::set_var("AOBATEST_PORT6", &p6);
        log::info!("🔗 Using virtual port override: AOBATEST_PORT6={p6}");
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
        log::info!("🧪 Test 2/3: TUI Master-Provide + CLI Slave-Poll - Repeat (10 rounds, holding registers)");
        tests::test_tui_master_with_cli_slave_continuous().await?;

        // Reset ports after test completes (Unix only)
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 2...");
            setup_virtual_serial_ports()?;
        }

        // Test 3: Multiple independent TUI masters and CLI slaves with interference checks
        log::info!("🧪 Test 3/3: Multiple Masters and Slaves + interference handling (5 rounds)");
        // Setup 6 ports for this test
        #[cfg(not(windows))]
        {
            log::info!("🧪 Setting up 6 virtual serial ports for Test 3...");
            setup_multiple_virtual_serial_ports()?;
        }
        tests::test_multiple_masters_slaves().await?;

        // Reset ports after Test 3 completes (Unix only) - back to 2 ports
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 3...");
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
