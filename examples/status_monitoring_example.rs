/// Example test demonstrating status monitoring for TUI E2E testing
///
/// This test shows how to use the new status monitoring utilities to verify
/// TUI/CLI behavior without relying on terminal screen capture.
use anyhow::Result;

use ci_utils::{read_tui_status, spawn_expect_process, wait_for_port_enabled, wait_for_tui_page};

#[tokio::test]
async fn test_status_monitoring_example() -> Result<()> {
    // This is an example test showing how to use status monitoring
    // It will be skipped in CI since it requires specific environment setup

    // Step 1: Enable debug mode by setting environment variable
    std::env::set_var("AOBA_DEBUG_CI_E2E_TEST", "1");

    // Step 2: Spawn TUI process
    let _tui_session = spawn_expect_process(&["--tui"])?;

    // Wait a bit for TUI to initialize and start writing status
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Step 3: Use status monitoring to verify TUI state

    // Wait for TUI to reach Entry page (with 10 second timeout)
    let status = wait_for_tui_page("Entry", 10, None).await?;
    log::info!("✅ TUI is on Entry page");
    log::info!(
        "   Ports available: {:?}",
        status.ports.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    // Read current TUI status directly
    let current_status = read_tui_status()?;
    log::info!("Current page: {}", current_status.page);
    log::info!("Number of ports: {}", current_status.ports.len());

    // Example: Wait for a specific port to be enabled (would timeout in this example)
    // let _status = wait_for_port_enabled("/tmp/vcom1", 5, None).await?;

    Ok(())
}
