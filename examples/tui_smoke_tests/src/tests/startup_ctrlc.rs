use anyhow::{anyhow, Result};

use expectrl::Expect;

use aoba::ci::{sleep_a_while, spawn_expect_process};

pub async fn test_tui_startup_ctrl_c_exit() -> Result<()> {
    let mut session = spawn_expect_process(&["--tui"])
        .map_err(|e| anyhow!("Failed to spawn TUI application: {}", e))?;

    session
        .send([3u8])
        .map_err(|err| anyhow!("Failed to send Ctrl+C: {}", err))?;
    sleep_a_while().await;

    log::info!("🧪 TUI startup and Ctrl+C exit test completed successfully");
    Ok(())
}
