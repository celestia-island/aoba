mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Inline of tests::runner::run_all()
    let _ = env_logger::try_init();
    log::info!("🧪 Starting TUI Integration Tests (User Simulation)...");

    tests::test_tui_startup_shutdown().await?;
    tests::test_tui_navigation().await?;
    tests::test_tui_serial_port_interaction().await?;

    log::info!("🧪 All TUI integration tests passed!");
    Ok(())
}
