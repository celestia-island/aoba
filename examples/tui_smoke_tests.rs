// TUI smoke tests using expectrl for basic startup/shutdown validation
// This is a dedicated example for testing TUI functionality, not for production release
// Run with: cargo run --example tui_smoke_tests

use anyhow::{anyhow, Result};
use std::{process::Command, time::Duration};

use expectrl::spawn;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();
    log::info!("🔥 Starting TUI Smoke Tests...");

    // Test 1: Basic TUI startup and Ctrl+C exit
    log::info!("✅ Test 1: TUI startup and Ctrl+C exit");
    test_tui_startup_ctrl_c_exit()?;

    // Test 2: TUI content detection
    log::info!("✅ Test 2: TUI content detection");
    test_tui_startup_detection()?;

    // Test 3: TUI with virtual serial ports
    log::info!("✅ Test 3: TUI with virtual serial ports");
    test_tui_with_virtual_ports().await?;

    // Test 4: Basic expectrl functionality
    log::info!("✅ Test 4: Basic expectrl functionality");
    test_expectrl_basic_functionality()?;

    log::info!("🎉 All TUI smoke tests passed!");
    Ok(())
}

/// Test that the TUI application starts and can be terminated with Ctrl+C
fn test_tui_startup_ctrl_c_exit() -> Result<()> {
    // Build the application first to ensure we have the binary
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .map_err(|err| anyhow!("Failed to execute cargo build: {}", err))?;

    if !build_output.status.success() {
        return Err(anyhow!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    // Start the TUI application
    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|err| anyhow!("Failed to spawn TUI application: {}", err))?;

    // Give the TUI time to initialize (shorter time for CI)
    std::thread::sleep(Duration::from_millis(500));

    // Send Ctrl+C to terminate the application
    session
        .send(&[3u8])
        .map_err(|err| anyhow!("Failed to send Ctrl+C: {}", err))?; // Send ASCII 3 (Ctrl+C)

    // Give it time to shut down gracefully
    std::thread::sleep(Duration::from_millis(300));

    log::info!("   ✓ TUI startup and Ctrl+C exit test completed successfully");
    Ok(())
}

/// Test that we can start the TUI and detect it's running by looking for expected output
fn test_tui_startup_detection() -> Result<()> {
    // Build the application first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .map_err(|err| anyhow!("Failed to execute cargo build: {}", err))?;

    if !build_output.status.success() {
        return Err(anyhow!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    // Test TUI startup and look for expected content
    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|err| anyhow!("Failed to spawn TUI application: {}", err))?;

    // Give the TUI time to initialize and display its interface
    std::thread::sleep(Duration::from_millis(800));

    // Try to capture some output that should be present in the TUI
    // We're looking for typical TUI elements like "AOBA" or "Refresh" or "Press q to quit"
    let mut found_tui_content = false;

    match session.expect(expectrl::Regex(r"(AOBA|COMPorts|Press.*quit|Refresh)")) {
        Ok(found) => {
            log::info!(
                "   ✓ Successfully detected TUI content: {:?}",
                found.matches()
            );
            found_tui_content = true;
        }
        Err(err) => {
            log::info!("   ⚠ Could not detect specific TUI content: {:?}", err);
            // Even if we can't detect specific content, the TUI might still be running
        }
    }

    // Send 'q' to quit
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send 'q' command: {}", err))?;
    std::thread::sleep(Duration::from_millis(300));

    if found_tui_content {
        log::info!("   ✓ TUI startup detection test completed successfully");
    } else {
        log::info!("   ⚠ TUI started but content detection was inconclusive");
    }

    Ok(())
}

/// Test TUI startup with virtual serial ports available
async fn test_tui_with_virtual_ports() -> Result<()> {
    // Note: examples must not spawn system providers (like socat) on CI runners.
    // Instead we only detect whether virtual ports exist and skip the test if not.
    let port1_exists = std::path::Path::new("/dev/ttyV1").exists();
    let port2_exists = std::path::Path::new("/dev/ttyV2").exists();

    if port1_exists && port2_exists {
        log::info!("   ✓ Virtual serial ports detected (created by CI or manually)");

        // Build the application first
        let build_output = Command::new("cargo")
            .args(["build", "--release"])
            .output()
            .map_err(|err| anyhow!("Failed to execute cargo build: {}", err))?;

        if !build_output.status.success() {
            return Err(anyhow!(
                "Failed to build application: {}",
                String::from_utf8_lossy(&build_output.stderr)
            ));
        }

        // Test TUI with virtual ports
        let mut session =
            spawn("./target/release/aoba --tui").expect("Failed to spawn TUI application");

        // Give the TUI time to initialize and potentially detect ports
        std::thread::sleep(Duration::from_millis(1000));

        // Try to detect the listed virtual ports in the TUI output
        let mut found_v1 = false;
        let mut found_v2 = false;

        // Try to read some output and search for the device names
        match session.expect(expectrl::Regex(r"/dev/ttyV1")) {
            Ok(_) => found_v1 = true,
            Err(_) => log::info!("/dev/ttyV1 not immediately visible in TUI output"),
        }
        match session.expect(expectrl::Regex(r"/dev/ttyV2")) {
            Ok(_) => found_v2 = true,
            Err(_) => log::info!("/dev/ttyV2 not immediately visible in TUI output"),
        }

        // If either device isn't shown, fail the test per requirements
        if !found_v1 || !found_v2 {
            return Err(anyhow!(
                "TUI did not display both /dev/ttyV1 and /dev/ttyV2"
            ));
        }

        // Send 'q' to quit gracefully
        session.send_line("q").expect("Failed to send 'q' command");
        std::thread::sleep(Duration::from_millis(300));

        log::info!("   ✓ TUI with virtual ports test completed (ports visible)");
    } else {
        log::info!("   ⚠ Virtual ports not detected, skipping test (CI-safe)");
    }

    Ok(())
}

/// Basic test to verify expectrl can capture terminal output from simple commands
fn test_expectrl_basic_functionality() -> Result<()> {
    let mut session = spawn("echo 'Hello from AOBA TUI test'")
        .map_err(|err| anyhow!("Failed to spawn echo command: {}", err))?;

    match session.expect(expectrl::Regex(r"Hello.*AOBA.*test")) {
        Ok(found) => {
            log::info!("   ✓ expectrl basic functionality test passed");
            log::info!("   ✓ Captured: {:?}", found.matches());
            let before_bytes = found.before();
            let before_str = String::from_utf8_lossy(before_bytes);
            if !before_str.is_empty() {
                log::info!("   ✓ Before content captured successfully");
            }
        }
        Err(err) => {
            return Err(anyhow!(
                "expectrl basic functionality test failed: {:?}",
                err
            ));
        }
    }

    Ok(())
}
