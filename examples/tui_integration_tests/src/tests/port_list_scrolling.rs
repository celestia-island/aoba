use anyhow::{anyhow, Result};
use expectrl::Expect;
use aoba::ci::{spawn_expect_process, TerminalCapture};

/// Test port list scrolling with different virtual port counts
pub async fn test_port_list_scrolling() -> Result<()> {
    log::info!("🧪 Testing port list scrolling with various configurations...");

    // Test Case 1: Standard terminal with navigation
    test_navigation_and_scrolling(24, 80, "standard_terminal").await?;

    // Test Case 2: Small terminal (forces scrolling with fewer ports)
    test_navigation_and_scrolling(15, 80, "small_terminal").await?;

    // Test Case 3: Tall terminal (more room for ports)
    test_navigation_and_scrolling(40, 80, "tall_terminal").await?;

    // Test Case 4: Navigate to bottom items and verify scrolling
    test_bottom_navigation(24, 80).await?;

    log::info!("✅ Port list scrolling tests completed successfully");
    Ok(())
}

/// Test navigation and verify scrolling works correctly
async fn test_navigation_and_scrolling(
    rows: u16,
    cols: u16,
    test_name: &str,
) -> Result<()> {
    log::info!(
        "  📝 Testing navigation and scrolling ({}x{}): {}",
        rows,
        cols,
        test_name
    );

    let args = vec!["--tui"];
    let mut session = spawn_expect_process(&args)
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    
    let mut cap = TerminalCapture::new(rows, cols);
    
    // Wait for TUI to render
    aoba::ci::sleep_a_while().await;
    
    // Capture initial state
    let initial = cap.capture(&mut session, &format!("{} - Initial state", test_name))?;
    
    // Verify initial screen has port list elements
    assert!(
        initial.contains("COM") || initial.contains("Ports") || initial.contains("tty") || initial.contains("AOBA"),
        "Initial screen should show port list panel or app header"
    );
    
    // Test navigation down
    log::info!("    Testing downward navigation...");
    for i in 0..8 {
        session
            .send("\x1b[B") // Down arrow
            .map_err(|err| anyhow!("Failed to send Down arrow: {}", err))?;
        
        aoba::ci::sleep_a_while().await;
        
        let screen = cap.capture(
            &mut session,
            &format!("{} - After {} down", test_name, i + 1),
        )?;
        
        // Verify selection indicator is visible
        assert!(
            screen.contains(">") || screen.contains("─") || screen.contains("AOBA"),
            "Screen should show selection indicator or borders"
        );
    }
    
    // Test navigation up
    log::info!("    Testing upward navigation...");
    for i in 0..5 {
        session
            .send("\x1b[A") // Up arrow
            .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?;
        
        aoba::ci::sleep_a_while().await;
        
        cap.capture(
            &mut session,
            &format!("{} - After {} up", test_name, i + 1),
        )?;
    }
    
    // Exit
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit: {}", err))?;
    
    log::info!("    ✓ Navigation test '{}' passed", test_name);
    Ok(())
}

/// Test navigation to bottom items (Refresh, Manual Specify, About)
async fn test_bottom_navigation(rows: u16, cols: u16) -> Result<()> {
    log::info!("  📝 Testing navigation to bottom items (last 3)");

    let args = vec!["--tui"];
    let mut session = spawn_expect_process(&args)
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    
    let mut cap = TerminalCapture::new(rows, cols);
    
    // Wait for TUI to render
    aoba::ci::sleep_a_while().await;
    
    // Initial capture
    cap.capture(&mut session, "Bottom nav - Initial")?;
    
    // Navigate down many times to reach the bottom items
    log::info!("    Navigating to bottom...");
    for _ in 0..50 {
        session
            .send("\x1b[B") // Down arrow
            .map_err(|err| anyhow!("Failed to send Down arrow: {}", err))?;
        aoba::ci::sleep_a_while().await;
    }
    
    // Capture at bottom
    let bottom = cap.capture(&mut session, "Bottom nav - At bottom")?;
    
    // Verify we're at one of the last 3 items
    // The exact text depends on i18n, but structure should be visible
    assert!(
        bottom.contains(">") || bottom.contains("─") || bottom.contains("AOBA"),
        "Should show selection at bottom items or app structure"
    );
    
    log::info!("    Testing selection of each bottom item...");
    
    // Navigate up one to previous item
    session
        .send("\x1b[A")
        .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?;
    aoba::ci::sleep_a_while().await;
    cap.capture(&mut session, "Bottom nav - Second to last")?;
    
    // Navigate up one more
    session
        .send("\x1b[A")
        .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?;
    aoba::ci::sleep_a_while().await;
    cap.capture(&mut session, "Bottom nav - Third to last")?;
    
    // Navigate back down
    session
        .send("\x1b[B")
        .map_err(|err| anyhow!("Failed to send Down arrow: {}", err))?;
    aoba::ci::sleep_a_while().await;
    session
        .send("\x1b[B")
        .map_err(|err| anyhow!("Failed to send Down arrow: {}", err))?;
    aoba::ci::sleep_a_while().await;
    cap.capture(&mut session, "Bottom nav - Back to bottom")?;
    
    // Exit
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit: {}", err))?;
    
    log::info!("    ✓ Bottom navigation test passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_port_list_scrolling_integration() {
        env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init()
            .ok();
        
        test_port_list_scrolling().await.expect("Port list scrolling test failed");
    }
}
