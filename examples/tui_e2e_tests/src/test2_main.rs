mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Running TUI E2E Test 2: CLI Master + TUI Slave");

    match tests::test_cli_master_with_tui_slave().await {
        Ok(_) => {
            log::info!("✅ Test 2 passed");
            Ok(())
        }
        Err(e) => {
            log::error!("❌ Test 2 failed: {}", e);
            Err(e)
        }
    }
}
