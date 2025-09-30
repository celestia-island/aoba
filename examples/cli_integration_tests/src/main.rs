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

    log::info!("🧪 All CLI integration tests passed!");
    Ok(())
}
