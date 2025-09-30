mod tests;

use anyhow::Result;

fn main() -> Result<()> {
    // Inline of tests::runner::run_all()
    // Initialize logger
    let _ = env_logger::try_init();
    log::info!("🧪 Starting CLI Integration Tests...");

    tests::test_cli_help()?;
    tests::test_cli_list_ports()?;
    tests::test_cli_list_ports_json()?;

    log::info!("🧪 All CLI integration tests passed!");
    Ok(())
}
