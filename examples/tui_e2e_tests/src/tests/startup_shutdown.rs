use anyhow::Result;

use aoba::ci::{ExpectKeyExt, spawn_expect_process, TerminalCapture};

pub async fn test_tui_startup_shutdown() -> Result<()> {
    let mut session = spawn_expect_process(&["--tui"])?;
    let mut cap = TerminalCapture::new(24, 80);
    let _ = cap.capture(&mut session, "Waiting for TUI to start")?;
    session.send_char('q')?;
    log::info!("🧪 TUI startup/shutdown test completed");
    Ok(())
}
