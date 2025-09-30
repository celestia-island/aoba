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

    // Deadlock regression tests (critical!)
    tests::test_navigation_to_refresh_no_deadlock().await?;
    tests::test_navigation_with_no_ports().await?;

    tests::test_port_list_scrolling().await?;
    tests::test_tui_serial_port_interaction().await?;

    log::info!("🧪 All TUI integration tests passed!");
    Ok(())
}
