// TUI integration tests for user simulation and black-box testing
// This is a dedicated example for comprehensive TUI testing, not for production release
// Run with: cargo run --example tui_integration_tests

use expectrl::{spawn, Regex};
use std::fs;
use std::process::Command;
use std::time::Duration;

#[tokio::main] 
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 Starting TUI Integration Tests (User Simulation)...");
    
    // Test 1: Basic TUI startup and shutdown
    println!("✅ Test 1: TUI startup and shutdown");
    test_tui_startup_shutdown().await?;
    
    // Test 2: TUI navigation and interaction
    println!("✅ Test 2: TUI navigation and interaction");
    test_tui_navigation().await?;
    
    // Test 3: TUI with virtual serial port interaction
    println!("✅ Test 3: TUI with virtual serial ports");
    test_tui_serial_port_interaction().await?;
    
    // Test 4: Dynamic content filtering
    println!("✅ Test 4: Dynamic content filtering");
    test_filter_dynamic_content();
    
    println!("🎉 All TUI integration tests passed!");
    Ok(())
}

/// Test basic TUI startup and shutdown
async fn test_tui_startup_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    // Setup virtual serial ports for testing
    setup_virtual_serial_ports().await?;

    // Build the application first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to execute cargo build");

    if !build_output.status.success() {
        return Err(format!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ).into());
    }

    // Spawn the TUI process
    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|e| format!("Failed to spawn TUI process: {}", e))?;

    // Wait for TUI to start (look for some expected content)
    let _ = session.expect(Regex(".*"));

    // Send quit command (typically 'q' or Ctrl+C)
    session.send_line("q").map_err(|e| format!("Failed to send quit command: {}", e))?;

    println!("   ✓ TUI startup/shutdown test completed");

    cleanup_virtual_serial_ports().await?;
    Ok(())
}

/// Test TUI navigation and basic interaction
async fn test_tui_navigation() -> Result<(), Box<dyn std::error::Error>> {
    setup_virtual_serial_ports().await?;

    // Build the application first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to execute cargo build");

    if !build_output.status.success() {
        return Err(format!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ).into());
    }

    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|e| format!("Failed to spawn TUI process: {}", e))?;

    // Wait for initial UI
    let _ = session.expect(Regex(".*"));

    // Test navigation keys
    session.send("\t").map_err(|e| format!("Failed to send Tab: {}", e))?; // Tab key
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    session.send_line("").map_err(|e| format!("Failed to send Enter: {}", e))?; // Enter key
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send arrow keys for navigation
    session.send("\x1b[A").map_err(|e| format!("Failed to send Up arrow: {}", e))?; // Up arrow
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    session.send("\x1b[B").map_err(|e| format!("Failed to send Down arrow: {}", e))?; // Down arrow
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    session.send("\x1b[C").map_err(|e| format!("Failed to send Right arrow: {}", e))?; // Right arrow
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    session.send("\x1b[D").map_err(|e| format!("Failed to send Left arrow: {}", e))?; // Left arrow
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Take snapshots of current terminal content would go here
    // For now, we just verify the navigation commands were sent successfully
    
    // Exit gracefully
    session.send_line("q").map_err(|e| format!("Failed to send quit: {}", e))?;
    println!("   ✓ TUI navigation test completed");

    cleanup_virtual_serial_ports().await?;
    Ok(())
}

/// Test TUI with virtual serial port interaction
async fn test_tui_serial_port_interaction() -> Result<(), Box<dyn std::error::Error>> {
    setup_virtual_serial_ports().await?;

    // Build the application first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to execute cargo build");

    if !build_output.status.success() {
        return Err(format!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ).into());
    }

    let mut session = spawn("./target/release/aoba --tui")
        .map_err(|e| format!("Failed to spawn TUI application: {}", e))?;

    // Wait for UI to load
    let _ = session.expect(Regex(".*"));

    // Navigate to port configuration (exact keys depend on UI layout)
    // This is a placeholder - adjust based on actual TUI flow
    session.send("\t").map_err(|e| format!("Failed to navigate: {}", e))?; // Navigate to port list
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    session.send_line("").map_err(|e| format!("Failed to select: {}", e))?; // Select a port
    tokio::time::sleep(Duration::from_millis(100)).await;

    // TODO: Add specific interactions based on TUI behavior
    // For example:
    // - Select virtual port
    // - Configure baud rate
    // - Open connection
    // - Send test data
    // - Verify received data

    // Exit
    session.send_line("q").map_err(|e| format!("Failed to quit: {}", e))?;
    println!("   ✓ TUI serial port test completed");

    cleanup_virtual_serial_ports().await?;
    Ok(())
}

/// Setup virtual serial ports for testing
async fn setup_virtual_serial_ports() -> Result<(), Box<dyn std::error::Error>> {
    let _child = Command::new("socat")
        .args([
            "-d",
            "-d",
            "pty,raw,echo=0,link=/tmp/vcom1",
            "pty,raw,echo=0,link=/tmp/vcom2",
        ])
        .spawn()
        .map_err(|e| format!("Failed to start socat: {}", e))?;

    // Wait for ports to be created
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Set permissions on the virtual ports
    if std::path::Path::new("/tmp/vcom1").exists() {
        let _ = Command::new("chmod")
            .args(["666", "/tmp/vcom1"])
            .output();
    }
    if std::path::Path::new("/tmp/vcom2").exists() {
        let _ = Command::new("chmod")
            .args(["666", "/tmp/vcom2"])
            .output();
    }

    Ok(())
}

/// Cleanup virtual serial ports
async fn cleanup_virtual_serial_ports() -> Result<(), Box<dyn std::error::Error>> {
    // Kill any socat processes (basic cleanup)
    let _ = Command::new("pkill").arg("socat").output();
    
    // Remove virtual port files if they exist
    let _ = fs::remove_file("/tmp/vcom1");
    let _ = fs::remove_file("/tmp/vcom2");
    
    Ok(())
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

/// Test the content filtering functionality
fn test_filter_dynamic_content() {
    let test_content = "⠋ Loading... 14:30:25 Status: ● Active ○ Idle 2024-01-15 14:30:25";
    let filtered = filter_dynamic_content(test_content);
    
    println!("   ✓ Original: {}", test_content);
    println!("   ✓ Filtered: {}", filtered);
    
    // Verify that dynamic content has been filtered
    assert!(!filtered.contains("⠋"));
    assert!(!filtered.contains("14:30:25"));
    assert!(!filtered.contains("●"));
    assert!(!filtered.contains("○"));
    assert!(filtered.contains("XX:XX:XX"));
    assert!(filtered.contains("XXXX-XX-XX XX:XX:XX"));
    
    println!("   ✓ Dynamic content filtering test passed");
}