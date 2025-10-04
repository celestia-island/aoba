mod tests;

use anyhow::Result;

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

    // Check if virtual serial ports are available for E2E tests
    if std::path::Path::new("/dev/vcom1").exists() && std::path::Path::new("/dev/vcom2").exists() {
        log::info!("🧪 Virtual serial ports detected, running E2E tests...");
        tests::test_slave_listen_with_vcom()?;
        tests::test_master_provide_with_vcom()?;
        tests::test_master_slave_communication()?;
    } else {
        log::warn!("⚠️ Virtual serial ports not found, skipping E2E tests");
    }

    log::info!("🧪 All CLI E2E tests passed!");
    Ok(())
}
