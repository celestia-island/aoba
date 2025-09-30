use anyhow::{anyhow, Result};

use expectrl::Expect;

use aoba::ci::{spawn_expect_process, TerminalCapture};

pub async fn test_tui_navigation() -> Result<()> {
    let mut session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    let mut cap = TerminalCapture::new(24, 80);
    let _ = cap.capture(&mut session, "Waiting for TUI to start")?;

    session
        .send("\t")
        .map_err(|err| anyhow!("Failed to send Tab: {}", err))?;

    session
        .send_line("")
        .map_err(|err| anyhow!("Failed to send Enter: {}", err))?;

    session
        .send("\x1b[A")
        .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?;
    session
        .send("\x1b[B")
        .map_err(|err| anyhow!("Failed to send Down arrow: {}", err))?;
    session
        .send("\x1b[C")
        .map_err(|err| anyhow!("Failed to send Right arrow: {}", err))?;
    session
        .send("\x1b[D")
        .map_err(|err| anyhow!("Failed to send Left arrow: {}", err))?;

    let _ = cap.capture(&mut session, "After navigation keys")?;
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit: {}", err))?;
    log::info!("🧪 TUI navigation test completed");
    Ok(())
}
