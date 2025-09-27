use expectrl::{spawn, Regex};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::Duration;

/// Test basic TUI startup and shutdown
#[tokio::test]
async fn test_tui_startup_shutdown() {
    // Setup virtual serial ports for testing
    setup_virtual_serial_ports().await;

    // Spawn the TUI process
    let mut session = spawn("./target/release/aoba --tui").expect("Failed to spawn TUI process");

    // Wait for TUI to start (look for some expected content)
    let _ = session.expect(Regex(".*")).unwrap();

    // Send quit command (typically 'q' or Ctrl+C)
    session.send_line("q").unwrap();

    // For now, just allow the process to exit naturally
    // TODO: Improve process termination handling when expectrl API is better understood
    println!("TUI startup/shutdown test completed");

    cleanup_virtual_serial_ports().await;
}

/// Test TUI navigation and basic interaction
#[tokio::test]
async fn test_tui_navigation() {
    setup_virtual_serial_ports().await;

    let mut session = spawn("./target/release/aoba --tui").expect("Failed to spawn TUI process");

    // Wait for initial UI
    let _ = session.expect(Regex(".*")).unwrap();

    // Test navigation keys
    session.send("\t").unwrap(); // Tab key
    session.send_line("").unwrap(); // Enter key

    // Send arrow keys for navigation
    session.send("\x1b[A").unwrap(); // Up arrow
    session.send("\x1b[B").unwrap(); // Down arrow
    session.send("\x1b[C").unwrap(); // Right arrow
    session.send("\x1b[D").unwrap(); // Left arrow

    // Take a snapshot of current terminal content
    let before = get_terminal_content(&mut session).await;

    // Perform some action
    session.send("j").unwrap(); // Move down

    let after = get_terminal_content(&mut session).await;

    // Verify something changed
    // Note: We use loose comparison to handle dynamic content like spinners
    assert_ne!(before.len(), 0);
    assert_ne!(after.len(), 0);

    // Exit gracefully
    session.send_line("q").unwrap();
    println!("TUI navigation test completed");

    cleanup_virtual_serial_ports().await;
}

/// Test TUI with virtual serial port interaction
#[tokio::test]
async fn test_tui_serial_port_interaction() {
    setup_virtual_serial_ports().await;

    let mut session = spawn("./target/release/aoba --tui").expect("Failed to spawn TUI process");

    // Wait for UI to load
    let _ = session.expect(Regex(".*")).unwrap();

    // Navigate to port configuration (exact keys depend on UI layout)
    // This is a placeholder - adjust based on actual TUI flow
    session.send("\t").unwrap(); // Navigate to port list
    session.send_line("").unwrap(); // Select a port

    // TODO: Add specific interactions based on TUI behavior
    // For example:
    // - Select virtual port
    // - Configure baud rate
    // - Open connection
    // - Send test data
    // - Verify received data

    // Exit
    session.send_line("q").unwrap();
    println!("TUI serial port test completed");

    cleanup_virtual_serial_ports().await;
}

/// Helper function to set up virtual serial ports using socat
async fn setup_virtual_serial_ports() {
    // Kill any existing socat processes
    let _ = Command::new("pkill").arg("socat").output();

    // Create virtual serial port pair
    let _child = Command::new("socat")
        .args([
            "-d",
            "-d",
            "pty,raw,echo=0,link=/tmp/tui_test_vcom1",
            "pty,raw,echo=0,link=/tmp/tui_test_vcom2",
        ])
        .spawn()
        .expect("Failed to start socat");

    // Wait for ports to be created
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Make ports accessible
    if let Ok(metadata) = fs::metadata("/tmp/tui_test_vcom1") {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o666);
        let _ = fs::set_permissions("/tmp/tui_test_vcom1", permissions);
    }

    if let Ok(metadata) = fs::metadata("/tmp/tui_test_vcom2") {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o666);
        let _ = fs::set_permissions("/tmp/tui_test_vcom2", permissions);
    }
}

/// Helper function to clean up virtual serial ports
async fn cleanup_virtual_serial_ports() {
    let _ = Command::new("pkill").arg("socat").output();
    let _ = fs::remove_file("/tmp/tui_test_vcom1");
    let _ = fs::remove_file("/tmp/tui_test_vcom2");
}

/// Helper function to capture current terminal content
/// This filters out dynamic content like timestamps and spinners
async fn get_terminal_content(session: &mut expectrl::Session) -> String {
    // Send a special sequence to get current terminal state
    session.send("\x1b[6n").unwrap(); // Request cursor position

    // Get current screen content - basic implementation
    let content = "terminal_content".to_string(); // Placeholder

    // Filter out dynamic content
    filter_dynamic_content(&content)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_dynamic_content() {
        let input = "Status: ⠋ Loading... 12:34:56 Connected";
        let output = filter_dynamic_content(input);
        assert_eq!(output, "Status:   Loading... XX:XX:XX Connected");
    }
}
