mod tests;

use anyhow::Result;
use std::process::Command;

/// Setup virtual serial ports by running socat_init script with sudo
fn setup_virtual_serial_ports() -> Result<bool> {
    log::info!("🧪 Setting up virtual serial ports...");

    // Find the socat_init.sh script
    let script_path = std::path::Path::new("examples/cli_e2e_tests/scripts/socat_init.sh");

    if !script_path.exists() {
        log::warn!(
            "⚠️ socat_init.sh script not found at {}",
            script_path.display()
        );
        return Ok(false);
    }

    // Run the script with sudo
    let output = Command::new("sudo").arg("bash").arg(script_path).output()?;

    if output.status.success() {
        log::info!("✅ Virtual serial ports setup successfully");
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

    // Setup virtual serial ports and run E2E tests
    if setup_virtual_serial_ports()? {
        log::info!("🧪 Virtual serial ports setup complete, running E2E tests...");
        tests::test_slave_listen_with_vcom()?;
        tests::test_master_provide_with_vcom()?;
        tests::test_master_slave_communication()?;
    } else {
        log::warn!("⚠️ Virtual serial ports setup failed, skipping E2E tests");
    }

    log::info!("🧪 All CLI E2E tests passed!");
    Ok(())
}
