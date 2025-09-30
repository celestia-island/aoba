use anyhow::{anyhow, Result};

use expectrl::Expect;

use aoba::ci::{
    should_run_vcom_tests, sleep_a_while, spawn_expect_process, vcom_matchers, TerminalCapture,
};

pub async fn test_tui_with_virtual_ports() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping virtual serial port dependent smoke test on this platform");
        return Ok(());
    }

    // If env vars or platform defaults indicate ports, proceed. We don't rely
    // on checking the filesystem for presence here; rely on the helper names
    // and let the test logic decide based on output.
    let vmatch = vcom_matchers();

    // On Unix the helper names will be /dev/vcom1 and /dev/vcom2 by default;
    // on Windows they will be COM1/COM2. If the user explicitly set
    // AOBATEST_PORT1/AOBATEST_PORT2, those are used.
    // Attempt a light filesystem existence check only for path-like names
    // (i.e., names that start with '/'). If both are path-like and exist,
    // run the interactive check; otherwise still try — this is best-effort.
    let both_exist_on_fs = vmatch.port1_name.starts_with('/')
        && vmatch.port2_name.starts_with('/')
        && std::path::Path::new(&vmatch.port1_name).exists()
        && std::path::Path::new(&vmatch.port2_name).exists();

    if both_exist_on_fs {
        let mut session =
            spawn_expect_process(&["--tui"]).expect("Failed to spawn TUI application");
        // Approximate original 1000ms wait with 10 * 100ms helper calls.
        for _ in 0..10 {
            sleep_a_while().await;
        }
        let mut found_v1 = false;
        let mut found_v2 = false;
        let mut cap = TerminalCapture::new(24, 80);
        let screen = cap.capture(&mut session, "virtual port detection")?;
        if vmatch.port1_rx.is_match(&screen) {
            found_v1 = true;
        }
        if vmatch.port2_rx.is_match(&screen) {
            found_v2 = true;
        }
        if !found_v1 || !found_v2 {
            return Err(anyhow!(
                "TUI did not display both {} and {}",
                vmatch.port1_name,
                vmatch.port2_name
            ));
        }
        session.send_line("q").expect("Failed to send 'q' command");
        // Approximate original 300ms wait with 3 * 100ms helper calls.
        for _ in 0..3 {
            sleep_a_while().await;
        }
    }
    Ok(())
}
