mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Starting TUI E2E Tests (End-to-End User Simulation)...");

    tests::test_navigation_to_refresh_no_deadlock().await?;
    tests::test_modbus_master_slave_communication().await?;

    log::info!("🧪 All TUI E2E tests passed!");
    Ok(())
}
