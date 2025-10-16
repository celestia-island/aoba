mod tests;

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

    // Run the script (no sudo required) to reset/reinitialize virtual serial ports
    let output = Command::new("bash")
        .arg(script_path)
        .arg("--mode")
        .arg("cli")
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

        log::info!("🧪 Starting CLI E2E Tests...");

        tests::test_cli_help().await?;
        tests::test_cli_list_ports().await?;
        tests::test_cli_list_ports_json().await?;
        tests::test_cli_list_ports_json_with_status().await?;

        log::info!("🧪 Testing Modbus CLI features (basic)...");
        tests::test_slave_listen_temp().await?;
        tests::test_slave_listen_persist().await?;
        tests::test_master_provide_temp().await?;
        tests::test_master_provide_persist().await?;

        // Check if we can setup virtual serial ports for E2E tests
        if setup_virtual_serial_ports()? {
            log::info!("🧪 Virtual serial ports available, running E2E tests...");

            // Run each E2E test with fresh port initialization
            log::info!("🧪 Test 1/7: Slave listen with virtual ports");
            setup_virtual_serial_ports()?;
            tests::test_slave_listen_with_vcom().await?;

            log::info!("🧪 Test 2/7: Master provide with virtual ports");
            setup_virtual_serial_ports()?;
            tests::test_master_provide_with_vcom().await?;

            log::info!("🧪 Test 3/7: Master-slave communication");
            setup_virtual_serial_ports()?;
            tests::test_master_slave_communication().await?;

            log::info!("🧪 Test 4/7: Basic master-slave communication");
            setup_virtual_serial_ports()?;
            tests::test_basic_master_slave_communication().await?;

            log::info!("🧪 Test 5/7: Configuration mode test");
            setup_virtual_serial_ports()?;
            tests::test_config_mode().await?;

            log::info!("🧪 Test 6/7: Multi-master configurations");
            setup_virtual_serial_ports()?;
            tests::test_multi_masters().await?;

            log::info!("🧪 Test 7/7: Multi-master same station configurations");
            setup_virtual_serial_ports()?;
            tests::test_multi_masters_same_station().await?;

            log::info!("🧪 Test 8/7: Multi-slave configurations");
            setup_virtual_serial_ports()?;
            tests::test_multi_slaves().await?;

            log::info!("🧪 Test 9/7: Multi-slave same station configurations");
            setup_virtual_serial_ports()?;
            tests::test_multi_slaves_same_station().await?;

            log::info!("🧪 Test 10/7: Multi-slave adjacent registers configurations");
            setup_virtual_serial_ports()?;
            tests::test_multi_slaves_adjacent_registers().await?;
        } else {
            log::warn!("⚠️ Virtual serial ports setup failed, skipping E2E tests");
        }

        if loop_count > 1 {
            log::info!("✅ Iteration {iteration}/{loop_count} completed successfully!");
        } else {
            log::info!("🧪 All CLI E2E tests passed!");
        }
    }

    if loop_count > 1 {
        log::info!("🎉 All {loop_count} iterations completed successfully!");
    }

    Ok(())
}
