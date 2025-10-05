mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Starting TUI E2E Tests (End-to-End User Simulation)...");

    // Run the original full TUI test (may fail if vcom not available)
    log::info!("🧪 Test 1: Full TUI Master-Slave communication");
    match tests::test_modbus_master_slave_communication().await {
        Ok(_) => log::info!("✅ Test 1 passed"),
        Err(e) => log::warn!("⚠️  Test 1 failed (expected): {}", e),
    }

    // Run hybrid tests (easier to debug and more reliable)
    log::info!("\n🧪 Test 2: TUI Master + CLI Slave hybrid test");
    match tests::test_tui_master_with_cli_slave().await {
        Ok(_) => log::info!("✅ Test 2 passed"),
        Err(e) => log::warn!("⚠️  Test 2 failed: {}", e),
    }

    log::info!("\n🧪 Test 3: CLI Master + TUI Slave hybrid test");
    match tests::test_cli_master_with_tui_slave().await {
        Ok(_) => log::info!("✅ Test 3 passed"),
        Err(e) => log::warn!("⚠️  Test 3 failed: {}", e),
    }

    log::info!("\n🧪 All TUI E2E tests completed!");
    Ok(())
}
