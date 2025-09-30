mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Starting TUI Integration Tests (User Simulation)...");

    tests::test_tui_startup_shutdown().await?;
    tests::test_tui_navigation().await?;
    tests::test_tui_serial_port_interaction().await?;

    log::info!("🧪 All TUI integration tests passed!");
    Ok(())
}
