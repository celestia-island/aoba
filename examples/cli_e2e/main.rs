mod tests;

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
    let output = Command::new("sudo").arg("bash").arg(script_path).output()?;

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

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Starting CLI E2E Tests...");

    tests::test_cli_help()?;
    tests::test_cli_list_ports()?;
    tests::test_cli_list_ports_json()?;
    tests::test_cli_list_ports_json_with_status()?;

    log::info!("🧪 Testing Modbus CLI features (basic)...");
    tests::test_slave_listen_temp()?;
    tests::test_slave_listen_persist()?;
    tests::test_master_provide_temp()?;
    tests::test_master_provide_persist()?;

    // Check if we can setup virtual serial ports for E2E tests
    if setup_virtual_serial_ports()? {
        log::info!("🧪 Virtual serial ports available, running E2E tests...");

        // Run each E2E test with fresh port initialization
        log::info!("🧪 Test 1/5: Slave listen with virtual ports");
        setup_virtual_serial_ports()?;
        tests::test_slave_listen_with_vcom()?;

        log::info!("🧪 Test 2/5: Master provide with virtual ports");
        setup_virtual_serial_ports()?;
        tests::test_master_provide_with_vcom()?;

        log::info!("🧪 Test 3/5: Master-slave communication");
        setup_virtual_serial_ports()?;
        tests::test_master_slave_communication()?;

        log::info!("🧪 Test 4/5: Continuous connection with files");
        setup_virtual_serial_ports()?;
        tests::test_continuous_connection_with_files()?;

        log::info!("🧪 Test 5/5: Continuous connection with pipes");
        setup_virtual_serial_ports()?;
        tests::test_continuous_connection_with_pipes()?;
    } else {
        log::warn!("⚠️ Virtual serial ports setup failed, skipping E2E tests");
    }

    log::info!("🧪 All CLI E2E tests passed!");
    Ok(())
}
