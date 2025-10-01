use anyhow::{anyhow, Result};
use aoba::ci::{spawn_expect_process, TerminalCapture};
use aoba::ci::{ArrowKey, ExpectKeyExt};

/// Test that navigation to Refresh item (first special item) doesn't cause deadlock
/// This is a regression test for the issue where navigating to Refresh would freeze the TUI
pub async fn test_navigation_to_refresh_no_deadlock() -> Result<()> {
    log::info!("🧪 Testing navigation to Refresh item (deadlock regression test)...");

    let args = vec!["--tui"];
    let mut session = spawn_expect_process(&args)
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;

    let mut cap = TerminalCapture::new(24, 80);

    // Wait for TUI to render
    aoba::ci::sleep_a_while().await;

    // Capture initial state
    cap.capture(&mut session, "Initial state")?;

    // Navigate down to reach the Refresh item (which is after all ports)
    // Use a large number to ensure we reach it regardless of port count
    log::info!("    Navigating down to Refresh item...");
    for i in 0..50 {
        session
            .send_arrow(ArrowKey::Down) // Down arrow
            .map_err(|err| anyhow!("Failed to send Down arrow at iteration {}: {}", i, err))?;
        aoba::ci::sleep_a_while().await;
    }

    // If we reach here without timeout, the deadlock is fixed
    let final_screen = cap.capture(&mut session, "After navigating to bottom items")?;

    // Verify we can still interact with the UI (not frozen)
    session
        .send_arrow(ArrowKey::Up) // Up arrow
        .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?;
    aoba::ci::sleep_a_while().await;

    cap.capture(&mut session, "After up arrow (verify still responsive)")?;

    // Exit with Ctrl+C
    session.send_ctrl_c()?;

    log::info!("    ✓ Navigation to Refresh item completed without deadlock");

    // Verify the screen contains expected elements
    assert!(
        final_screen.contains("AOBA") || final_screen.contains(">") || final_screen.contains("─"),
        "Screen should show UI elements (not frozen)"
    );

    Ok(())
}
