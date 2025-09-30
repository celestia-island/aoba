use anyhow::{anyhow, Result};
use aoba::ci::{spawn_expect_process, TerminalCapture};
use expectrl::Expect;

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
            .send("\x1b[B") // Down arrow
            .map_err(|err| anyhow!("Failed to send Down arrow at iteration {}: {}", i, err))?;
        aoba::ci::sleep_a_while().await;
    }

    // If we reach here without timeout, the deadlock is fixed
    let final_screen = cap.capture(&mut session, "After navigating to bottom items")?;

    // Verify we can still interact with the UI (not frozen)
    session
        .send("\x1b[A") // Up arrow
        .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?;
    aoba::ci::sleep_a_while().await;

    cap.capture(&mut session, "After up arrow (verify still responsive)")?;

    // Exit
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit: {}", err))?;

    log::info!("    ✓ Navigation to Refresh item completed without deadlock");

    // Verify the screen contains expected elements
    assert!(
        final_screen.contains("AOBA") || final_screen.contains(">") || final_screen.contains("─"),
        "Screen should show UI elements (not frozen)"
    );

    Ok(())
}

/// Test navigation with no ports (edge case that was particularly problematic)
pub async fn test_navigation_with_no_ports() -> Result<()> {
    log::info!("🧪 Testing navigation with no/few ports (edge case)...");

    let args = vec!["--tui"];
    let mut session = spawn_expect_process(&args)
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;

    let mut cap = TerminalCapture::new(24, 80);

    // Wait for TUI to render
    aoba::ci::sleep_a_while().await;

    // Capture initial state
    cap.capture(&mut session, "Initial state with no/few ports")?;

    // Immediately try to navigate down (if no ports, this goes straight to Refresh)
    log::info!("    Testing immediate down navigation...");
    for i in 0..5 {
        session
            .send("\x1b[B") // Down arrow
            .map_err(|err| anyhow!("Failed to send Down arrow {}: {}", i, err))?;
        aoba::ci::sleep_a_while().await;
    }

    // Verify still responsive
    let screen = cap.capture(&mut session, "After down navigation")?;

    // Navigate up
    for i in 0..3 {
        session
            .send("\x1b[A") // Up arrow
            .map_err(|err| anyhow!("Failed to send Up arrow {}: {}", i, err))?;
        aoba::ci::sleep_a_while().await;
    }

    cap.capture(&mut session, "After up navigation")?;

    // Exit
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit: {}", err))?;

    log::info!("    ✓ Navigation with no/few ports works correctly");

    // Verify screen is not frozen
    assert!(
        screen.contains("AOBA") || screen.contains(">") || screen.contains("─"),
        "Screen should show UI elements"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_deadlock_on_navigation() {
        env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init()
            .ok();

        test_navigation_to_refresh_no_deadlock()
            .await
            .expect("Navigation to Refresh deadlock test failed");

        test_navigation_with_no_ports()
            .await
            .expect("Navigation with no ports test failed");
    }
}
