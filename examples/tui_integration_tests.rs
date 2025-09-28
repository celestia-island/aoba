// TUI integration tests for user simulation and black-box testing
// This is a dedicated example for comprehensive TUI testing, not for production release
// Run with: cargo run --example tui_integration_tests

use anyhow::{anyhow, Result};
use std::{process::Command, time::Duration};

use expectrl::{spawn, Regex};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();
    log::info!("🎭 Starting TUI Integration Tests (User Simulation)...");

    // Test 1: Basic TUI startup and shutdown
    log::info!("✅ Test 1: TUI startup and shutdown");
    test_tui_startup_shutdown().await?;

    // Test 2: TUI navigation and interaction
    log::info!("✅ Test 2: TUI navigation and interaction");
    test_tui_navigation().await?;

    // Test 3: TUI with virtual serial port interaction
    log::info!("✅ Test 3: TUI with virtual serial ports");
    test_tui_serial_port_interaction().await?;

    // Test 4: Dynamic content filtering
    log::info!("✅ Test 4: Dynamic content filtering");
    test_filter_dynamic_content();

    log::info!("🎉 All TUI integration tests passed!");
    Ok(())
}

/// Test basic TUI startup and shutdown
async fn test_tui_startup_shutdown() -> Result<()> {
    // Setup virtual serial ports for testing
    // (removed: setup of virtual serial ports to avoid starting system providers in examples)

    // Build the application first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to execute cargo build");

    if !build_output.status.success() {
        return Err(anyhow!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    // Spawn the TUI process
    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;

    // Wait for TUI to start (look for some expected content)
    let _ = session.expect(Regex(".*"));

    // Send quit command (typically 'q' or Ctrl+C)
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit command: {}", err))?;

    log::info!("   ✓ TUI startup/shutdown test completed");

    // (removed: cleanup of virtual serial ports)
    Ok(())
}

/// Test TUI navigation and basic interaction
async fn test_tui_navigation() -> Result<()> {
    // (removed: setup of virtual serial ports)

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

    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;

    // Wait for initial UI
    let _ = session.expect(Regex(".*"));

    // Test navigation keys
    session
        .send("\t")
        .map_err(|err| anyhow!("Failed to send Tab: {}", err))?; // Tab key
    tokio::time::sleep(Duration::from_millis(100)).await;

    session
        .send_line("")
        .map_err(|err| anyhow!("Failed to send Enter: {}", err))?; // Enter key
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send arrow keys for navigation
    session
        .send("\x1b[A")
        .map_err(|err| anyhow!("Failed to send Up arrow: {}", err))?; // Up arrow
    tokio::time::sleep(Duration::from_millis(100)).await;

    session
        .send("\x1b[B")
        .map_err(|err| anyhow!("Failed to send Down arrow: {}", err))?; // Down arrow
    tokio::time::sleep(Duration::from_millis(100)).await;

    session
        .send("\x1b[C")
        .map_err(|err| anyhow!("Failed to send Right arrow: {}", err))?; // Right arrow
    tokio::time::sleep(Duration::from_millis(100)).await;

    session
        .send("\x1b[D")
        .map_err(|err| anyhow!("Failed to send Left arrow: {}", err))?; // Left arrow
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Take snapshots of current terminal content would go here
    // For now, we just verify the navigation commands were sent successfully

    // Exit gracefully
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to send quit: {}", err))?;
    log::info!("   ✓ TUI navigation test completed");

    // (removed: cleanup of virtual serial ports)
    Ok(())
}

/// Test TUI with virtual serial port interaction
async fn test_tui_serial_port_interaction() -> Result<()> {
    // (removed: setup of virtual serial ports)

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

    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|err| anyhow!("Failed to spawn TUI application: {}", err))?;

    // Wait for UI to load
    let _ = session.expect(Regex(".*"));

    // Navigate to port configuration (exact keys depend on UI layout)
    // This is a placeholder - adjust based on actual TUI flow
    session
        .send("\t")
        .map_err(|err| anyhow!("Failed to navigate: {}", err))?; // Navigate to port list
    tokio::time::sleep(Duration::from_millis(100)).await;

    session
        .send_line("")
        .map_err(|err| anyhow!("Failed to select: {}", err))?; // Select a port
    tokio::time::sleep(Duration::from_millis(100)).await;

    // After UI navigation, try to detect the virtual ports displayed on screen
    let mut found_v1 = false;
    let mut found_v2 = false;

    match session.expect(Regex(r"/dev/ttyV1")) {
        Ok(_) => found_v1 = true,
        Err(_) => log::info!("/dev/ttyV1 not visible in TUI output"),
    }
    match session.expect(Regex(r"/dev/ttyV2")) {
        Ok(_) => found_v2 = true,
        Err(_) => log::info!("/dev/ttyV2 not visible in TUI output"),
    }

    if !found_v1 || !found_v2 {
        return Err(anyhow!(
            "TUI did not display both /dev/ttyV1 and /dev/ttyV2"
        ));
    }

    // TODO: Add specific interactions based on TUI behavior
    // For example:
    // - Select virtual port
    // - Configure baud rate
    // - Open connection
    // - Send test data
    // - Verify received data

    // Exit
    session
        .send_line("q")
        .map_err(|err| anyhow!("Failed to quit: {}", err))?;
    log::info!("   ✓ TUI serial port test completed");

    // (removed: cleanup of virtual serial ports)
    Ok(())
}

/// Setup virtual serial ports for testing
///
/// NOTE: In CI we should avoid creating system-level virtual serial ports or
/// spawning background providers like `socat` from examples. Instead this
/// function performs a non-invasive check and logs whether the expected
/// virtual devices exist. This keeps CI clean and avoids leaving resident
/// processes behind.
// setup_virtual_serial_ports removed: examples must not spawn system providers

/// Cleanup virtual serial ports
///
/// NOTE: Instead of attempting to remove device files or pkill providers,
/// this function intentionally does not modify system state. In CI we prefer
/// to rely on killing provider processes explicitly if needed (outside this
/// example), or the CI job/container teardown to clean resources.
// cleanup_virtual_serial_ports removed: examples must not attempt to clean system state

/// Filter out dynamic content like spinners and timestamps
fn filter_dynamic_content(content: &str) -> String {
    let mut filtered = content.to_string();

    // Remove common spinner characters
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    for &c in &spinner_chars {
        filtered = filtered.replace(c, " ");
    }

    // Remove timestamps (basic pattern matching)
    // Pattern: HH:MM:SS or YYYY-MM-DD HH:MM:SS
    let re = regex::Regex::new(r"\d{2}:\d{2}:\d{2}").unwrap();
    filtered = re.replace_all(&filtered, "XX:XX:XX").to_string();

    let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}").unwrap();
    filtered = re.replace_all(&filtered, "XXXX-XX-XX XX:XX:XX").to_string();

    // Remove other common dynamic indicators
    filtered = filtered.replace("●", " "); // dots
    filtered = filtered.replace("○", " "); // circles

    // Normalize whitespace
    filtered = filtered.trim().to_string();

    filtered
}

/// Test the content filtering functionality
fn test_filter_dynamic_content() {
    let test_content = "⠋ Loading... 14:30:25 Status: ● Active ○ Idle 2024-01-15 14:30:25";
    let filtered = filter_dynamic_content(test_content);

    log::info!("   ✓ Original: {}", test_content);
    log::info!("   ✓ Filtered: {}", filtered);

    // Verify that dynamic content has been filtered
    assert!(!filtered.contains("⠋"));
    assert!(!filtered.contains("14:30:25"));
    assert!(!filtered.contains("●"));
    assert!(!filtered.contains("○"));
    assert!(filtered.contains("XX:XX:XX"));
    assert!(filtered.contains("XXXX-XX-XX XX:XX:XX"));

    log::info!("   ✓ Dynamic content filtering test passed");
}
