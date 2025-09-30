use anyhow::{anyhow, Result};

use aoba::ci::{spawn_expect_process, ArrowKey, ExpectKeyExt, TerminalCapture};

pub async fn test_tui_navigation() -> Result<()> {
    let mut session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    let mut cap = TerminalCapture::new(24, 80);
    let _ = cap.capture(&mut session, "Waiting for TUI to start")?;

    // Navigate all the way down to the bottom (About page)
    log::info!("🧪 Navigating down to bottom items...");
    for i in 0..50 {
        session.send_arrow(ArrowKey::Down)?;
        if i % 10 == 0 {
            aoba::ci::sleep_a_while().await;
        }
    }

    let screen = cap.capture(&mut session, "At bottom of list")?;
    log::info!("Screen at bottom:\n{}", screen);

    // Press Enter to go to About page
    log::info!("🧪 Entering About page...");
    session.send_enter()?;
    aoba::ci::sleep_a_while().await;

    let about_screen = cap.capture(&mut session, "In About page")?;
    log::info!("About page:\n{}", about_screen);

    // Press Escape to return to main page
    log::info!("🧪 Pressing Escape to return to main page...");
    session.send_escape()?;
    aoba::ci::sleep_a_while().await;

    let main_screen = cap.capture(&mut session, "Back to main page")?;
    log::info!("Back to main page:\n{}", main_screen);

    // Quit
    session.send_char('q')?;
    log::info!("🧪 TUI navigation test completed");
    Ok(())
}
