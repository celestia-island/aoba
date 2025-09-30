// This E2E test checks that the TUI lists two virtual serial ports and
// allows selecting one. On Unix CI we expect `/dev/vcom1` and `/dev/vcom2`
// (created by the CI using socat or similar). On Windows there is no `/dev`
// namespace; users running these tests locally should prepare `COM1` and
// `COM2` (for example by creating paired virtual serial ports using a
// tool like com0com, or by connecting two real serial devices). The test
// will match COM1/COM2 on Windows and /dev/vcom1/2 on Unix.
//
// Note: by default vcom tests are only enabled on Unix. To force them on
// non-Unix platforms set the environment variable `CI_FORCE_VCOM=1`.
use anyhow::{anyhow, Result};
use regex::Regex;

use aoba::ci::{ArrowKey, ExpectKeyExt};
use aoba::ci::{should_run_vcom_tests, spawn_expect_process, vcom_matchers, TerminalCapture};

pub async fn test_tui_serial_port_interaction() -> Result<()> {
    let mut session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI application: {}", err))?;

    aoba::ci::sleep_a_while().await;
    let mut cap = TerminalCapture::new(24, 80);
    let screen = cap.capture(&mut session, "TUI startup")?;

    if !should_run_vcom_tests() {
        log::info!("Skipping virtual serial port interaction checks on this platform");
        return Ok(());
    }

    // Use centralized helper to obtain platform-appropriate regexes and names.
    let vmatch = vcom_matchers();
    let mut found_v1 = false;
    let mut found_v2 = false;
    if vmatch.port1_rx.is_match(&screen) {
        found_v1 = true;
        log::info!("🧪 Found virtual port in TUI output: {}", vmatch.port1_name);
    }
    if vmatch.port2_rx.is_match(&screen) {
        found_v2 = true;
        log::info!("🧪 Found virtual port in TUI output: {}", vmatch.port2_name);
    }

    if !found_v1 || !found_v2 {
        let _ = cap.capture(&mut session, "port detection failure")?;
        return Err(anyhow!(
            "TUI did not display both expected virtual ports: {} and {}",
            vmatch.port1_name,
            vmatch.port2_name
        ));
    }

    let cursor_rx = vmatch.cursor_rx;
    for i in 0..10 {
        session.send_arrow(ArrowKey::Up)?;
        let screen = cap.capture(&mut session, &format!("up arrow press #{}", i + 1))?;
        if cursor_rx.is_match(&screen) {
            log::info!("🧪 Cursor found at virtual port after {} up presses", i + 1);
            let _ = cap.capture(&mut session, "cursor found at virtual port")?;
            break;
        }
    }

    log::info!("🧪 Pressing Enter to select the port...");
    session.send_enter()?;

    aoba::ci::sleep_a_while().await;
    let _ = cap.capture(&mut session, "Enter key press")?;

    match session.send_char('q') {
        Ok(_) => {
            aoba::ci::sleep_a_while().await;
            let screen = cap.capture(&mut session, "q key press for testing responsiveness")?;
            if Regex::new(r"(?i)(error|failed|panic|crash)")
                .unwrap()
                .is_match(&screen)
            {
                return Err(anyhow!(
                    "TUI interaction test detected errors or unresponsive behavior"
                ));
            }
        }
        Err(_err) => {
            let _ = cap.capture(&mut session, "application unresponsive")?;
            return Err(anyhow!(
                "TUI application crashed or became unresponsive after pressing Enter"
            ));
        }
    }

    log::info!("🧪 TUI serial port interaction test completed successfully");
    Ok(())
}
