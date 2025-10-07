mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Running TUI E2E Test 1: TUI Master + CLI Slave");

    match tests::test_tui_master_with_cli_slave().await {
        Ok(_) => {
            log::info!("✅ Test 1 passed");
            Ok(())
        }
        Err(e) => {
            log::error!("❌ Test 1 failed: {}", e);
            Err(e)
        }
    }
}
