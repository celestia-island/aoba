mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Starting TUI E2E Tests (Hybrid CLI+TUI)...");

    // Run hybrid tests (easier to debug and more reliable)
    log::info!("🧪 Test 1: TUI Master + CLI Slave hybrid test");
    match tests::test_tui_master_with_cli_slave().await {
        Ok(_) => log::info!("✅ Test 1 passed"),
        Err(e) => {
            log::error!("❌ Test 1 failed: {}", e);
            return Err(e);
        }
    }

    log::info!("\n🧪 Test 2: CLI Master + TUI Slave hybrid test");
    match tests::test_cli_master_with_tui_slave().await {
        Ok(_) => log::info!("✅ Test 2 passed"),
        Err(e) => {
            log::error!("❌ Test 2 failed: {}", e);
            return Err(e);
        }
    }

    log::info!("\n🎉 All TUI E2E hybrid tests passed!");
    Ok(())
}
