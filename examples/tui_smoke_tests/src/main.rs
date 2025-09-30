mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Inline of tests::runner::run_all()
    let _ = env_logger::try_init();
    log::info!("🧪 Starting TUI Smoke Tests...");

    tests::test_tui_startup_ctrl_c_exit().await?;
    tests::test_tui_startup_detection().await?;
    tests::test_tui_with_virtual_ports().await?;

    log::info!("🧪 All TUI smoke tests passed!");
    Ok(())
}
