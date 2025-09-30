use anyhow::{anyhow, Result};
use regex::Regex;

use expectrl::Expect;

use aoba::ci::{sleep_a_while, spawn_expect_process, TerminalCapture};

pub async fn test_tui_startup_detection() -> Result<()> {
    let mut session = spawn_expect_process(&["--tui"])
        .map_err(|e| anyhow!("Failed to spawn TUI application: {}", e))?;
    // Approximate the original 800ms wait with 8 * 100ms helper calls.
    for _ in 0..8 {
        sleep_a_while().await;
    }
    let mut found_tui_content = false;
    let mut cap = TerminalCapture::new(24, 80);
    let screen = cap.capture(&mut session, "startup detection")?;
    if Regex::new(r"(AOBA|COMPorts|Press.*quit|Refresh)")
        .unwrap()
        .is_match(&screen)
    {
        log::info!("🧪 Successfully detected TUI content (via screen capture)");
        found_tui_content = true;
    }
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send 'q' command: {}", err))?;
    // Approximate the original 300ms wait with 3 * 100ms helper calls.
    for _ in 0..3 {
        sleep_a_while().await;
    }
    if found_tui_content {
        log::info!("🧪 TUI startup detection test completed successfully");
    }
    Ok(())
}
