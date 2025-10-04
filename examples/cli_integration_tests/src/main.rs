mod tests;

use anyhow::Result;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Starting CLI Integration Tests...");

    tests::test_cli_help()?;
    tests::test_cli_list_ports()?;
    tests::test_cli_list_ports_json()?;
    tests::test_cli_list_ports_json_with_status()?;
    
    log::info!("🧪 Testing Modbus CLI features...");
    tests::test_slave_listen_temp()?;
    tests::test_slave_listen_persist()?;
    tests::test_master_provide_temp()?;
    tests::test_master_provide_persist()?;

    log::info!("🧪 All CLI integration tests passed!");
    Ok(())
}
