use anyhow::{anyhow, Result};


use aoba::ci::{spawn_expect_process, TerminalCapture};
use super::key_input::{ArrowKey, ExpectKeyExt};

pub async fn test_tui_navigation() -> Result<()> {
    let mut session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    let mut cap = TerminalCapture::new(24, 80);
    let _ = cap.capture(&mut session, "Waiting for TUI to start")?;

    session.send_tab()?;
    session.send_enter()?;

    session.send_arrow(ArrowKey::Up)?;
    session.send_arrow(ArrowKey::Down)?;
    session.send_arrow(ArrowKey::Right)?;
    session.send_arrow(ArrowKey::Left)?;

    let _ = cap.capture(&mut session, "After navigation keys")?;
    session.send_char('q')?;
    log::info!("🧪 TUI navigation test completed");
    Ok(())
}
