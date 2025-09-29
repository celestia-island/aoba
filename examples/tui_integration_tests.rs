// TUI integration tests for user simulation and black-box testing
// This is a dedicated example for comprehensive TUI testing, not for production release
// Run with: cargo run --example tui_integration_tests

use anyhow::{anyhow, Result};
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use expectrl::{spawn, Expect};
use regex::Regex;

use aoba::test_utils::TerminalCapture;

#[tokio::main]
async fn main() -> Result<()> {
    // Set RUST_LOG to info if not already set
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::try_init()?;
    log::info!("🧪 Starting TUI Integration Tests (User Simulation)...");
    // Build the application once at startup and reuse the binary path for all tests
    log::info!("🧪 Building application once for all integration tests...");

    // Spawn cargo build and let it inherit the parent's stdio so output goes directly to this process's
    // stdout/stderr (useful for CI to capture build logs as-is).
    let status = Command::new("cargo")
        .args(["build"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| anyhow!("Failed to execute cargo build: {}", e))?;

    if !status.success() {
        return Err(anyhow!("cargo build failed with status: {}", status));
    }

    // Path to the built binary; on Windows the executable has .exe
    #[cfg(windows)]
    let bin_path = "./target/debug/aoba.exe";
    #[cfg(not(windows))]
    let bin_path = "./target/debug/aoba";

    log::info!("🧪 Test 1: TUI startup and shutdown");
    test_tui_startup_shutdown(bin_path).await?;

    log::info!("🧪 Test 2: TUI navigation and interaction");
    test_tui_navigation(bin_path).await?;

    log::info!("🧪 Test 3: TUI with virtual serial ports");
    test_tui_serial_port_interaction(bin_path).await?;

    log::info!("🧪 All TUI integration tests passed!");
    Ok(())
}

/// Test basic TUI startup and shutdown
async fn test_tui_startup_shutdown(bin_path: &str) -> Result<()> {
    // Spawn the TUI process
    let spawn_cmd = format!("{} --tui", bin_path);
    let mut session =
        spawn(spawn_cmd).map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    // Wait for TUI to start (look for some expected content)
    let mut cap = TerminalCapture::new(24, 80);
    let _ = cap.capture(&mut session, "Waiting for TUI to start")?;

    // Send quit command (typically 'q' or Ctrl+C)
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit command: {}", err))?;

    log::info!("🧪 TUI startup/shutdown test completed");
    Ok(())
}

/// Test TUI navigation and basic interaction
async fn test_tui_navigation(bin_path: &str) -> Result<()> {
    let spawn_cmd = format!("{} --tui", bin_path);
    let mut session =
        spawn(spawn_cmd).map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    // Wait for initial UI
    let mut cap = TerminalCapture::new(24, 80);
    let _ = cap.capture(&mut session, "Waiting for TUI to start")?;

    // Test navigation keys
    session
        .send("\t")
        .map_err(|err| anyhow!("Failed to send Tab: {}", err))?; // Tab key

    session
        .send_line("")
        .map_err(|err| anyhow!("Failed to send Enter: {}", err))?; // Enter key

    // Send arrow keys for navigation
    session
        .send("\x1b[A")
        .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?; // Up arrow

    session
        .send("\x1b[B")
        .map_err(|err| anyhow!("Failed to send Down arrow: {}", err))?; // Down arrow

    session
        .send("\x1b[C")
        .map_err(|err| anyhow!("Failed to send Right arrow: {}", err))?; // Right arrow

    session
        .send("\x1b[D")
        .map_err(|err| anyhow!("Failed to send Left arrow: {}", err))?; // Left arrow

    // Take snapshots of current terminal content would go here
    let _ = cap.capture(&mut session, "After navigation keys")?;

    // Exit gracefully
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit: {}", err))?;
    log::info!("🧪 TUI navigation test completed");
    Ok(())
}

/// Test TUI with virtual serial port interaction
async fn test_tui_serial_port_interaction(bin_path: &str) -> Result<()> {
    let spawn_cmd = format!("{} --tui", bin_path);
    let mut session =
        spawn(spawn_cmd).map_err(|err| anyhow!("Failed to spawn TUI application: {}", err))?;

    // Wait for UI to load
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Capture initial screen state
    let mut cap = TerminalCapture::new(24, 80);
    let screen = cap.capture(&mut session, "TUI startup")?;

    // First check if virtual ports are visible in the output
    let mut found_v1 = false;
    let mut found_v2 = false;

    // Try to capture current screen content
    if Regex::new(r"/dev/vcom1").unwrap().is_match(&screen) {
        found_v1 = true;
        log::info!("🧪 Found /dev/vcom1 in TUI output");
    } else {
        log::info!("🧪 /dev/vcom1 not visible in TUI output");
    }

    if Regex::new(r"/dev/vcom2").unwrap().is_match(&screen) {
        found_v2 = true;
        log::info!("🧪 Found /dev/vcom2 in TUI output");
    } else {
        log::info!("🧪 /dev/vcom2 not visible in TUI output");
    }

    if !found_v1 || !found_v2 {
        let _ = cap.capture(&mut session, "port detection failure")?;
        return Err(anyhow!(
            "TUI did not display both /dev/vcom1 and /dev/vcom2"
        ));
    }

    // Navigate to first item using up arrow keys
    // Keep pressing up until we reach the first item (cursor should be at index 0)
    log::info!("🧪 Navigating to first item using up arrow keys...");

    // Press up multiple times to ensure we reach the first item
    for i in 0..10 {
        // Send up arrow key as escape sequence
        session
            .send("\x1b[A") // Up arrow escape sequence
            .map_err(|err| anyhow!("Failed to send up arrow: {}", err))?;

        // Capture screen content after each key press
        let screen = cap.capture(&mut session, &format!("up arrow press #{}", i + 1))?;

        // Check if we can find the cursor indicator at the first item
        // Look for "> /dev/vcom" pattern indicating cursor is on a virtual port
        if Regex::new(r"> /dev/vcom[12]").unwrap().is_match(&screen) {
            log::info!("🧪 Cursor found at virtual port after {} up presses", i + 1);
            let _ = cap.capture(&mut session, "cursor found at virtual port")?;
            break;
        } else {
            if i == 9 {
                log::warn!("Could not locate cursor at first virtual port after 10 attempts");
                let _ = cap.capture(&mut session, "final navigation attempt")?;
            }
        }
    }

    // Press Enter to select the port
    log::info!("🧪 Pressing Enter to select the port...");
    session
        .send("\r") // Enter key
        .map_err(|err| anyhow!("Failed to send Enter: {}", err))?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Capture screen after Enter press
    let _ = cap.capture(&mut session, "Enter key press")?;

    // Check for crashes or error messages
    // If the application crashed, the session would be terminated
    // If there are error messages, they would typically contain keywords like "error", "failed", "panic"

    let mut has_errors = false;

    // Try to interact with the session to see if it's still responsive
    match session.send("q") {
        Ok(_) => {
            log::info!("🧪 Application still responsive after Enter press");
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Capture screen after q press
            let screen = cap.capture(&mut session, "q key press for testing responsiveness")?;

            // Check if we can capture any error messages in the output by
            // running a regex against the captured screen string.
            if Regex::new(r"(?i)(error|failed|panic|crash)")
                .unwrap()
                .is_match(&screen)
            {
                has_errors = true;
                log::warn!("🧪 Detected error-like messages in TUI output");
                let _ = cap.capture(&mut session, "error detection")?;
            } else {
                log::info!("🧪 No error messages detected in TUI output");
            }
        }
        Err(err) => {
            log::error!("🧪 Application became unresponsive: {err}");
            let _ = cap.capture(&mut session, "application unresponsive")?;
            return Err(anyhow!(
                "TUI application crashed or became unresponsive after pressing Enter"
            ));
        }
    }

    // Final quit attempt
    match session.send("q") {
        Ok(_) => {
            tokio::time::sleep(Duration::from_millis(300)).await;
            log::info!("🧪 TUI exited gracefully");
            let _ = cap.capture(&mut session, "final quit attempt")?;
        }
        Err(_) => {
            log::info!("🧪 Application may have already exited");
        }
    }

    if has_errors {
        return Err(anyhow!(
            "TUI interaction test detected errors or unresponsive behavior"
        ));
    }

    log::info!("🧪 TUI serial port interaction test completed successfully");
    Ok(())
}
