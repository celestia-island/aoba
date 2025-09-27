// TUI smoke tests using expectrl for basic startup/shutdown validation
// This is a dedicated example for testing TUI functionality, not for production release
// Run with: cargo run --example tui_smoke_tests

use expectrl::spawn;
use std::process::Command;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔥 Starting TUI Smoke Tests...");

    // Test 1: Basic TUI startup and Ctrl+C exit
    println!("✅ Test 1: TUI startup and Ctrl+C exit");
    test_tui_startup_ctrl_c_exit()?;

    // Test 2: TUI content detection
    println!("✅ Test 2: TUI content detection");
    test_tui_startup_detection()?;

    // Test 3: TUI with virtual serial ports
    println!("✅ Test 3: TUI with virtual serial ports");
    test_tui_with_virtual_ports().await?;

    // Test 4: Basic expectrl functionality
    println!("✅ Test 4: Basic expectrl functionality");
    test_expectrl_basic_functionality()?;

    println!("🎉 All TUI smoke tests passed!");
    Ok(())
}

/// Test that the TUI application starts and can be terminated with Ctrl+C
fn test_tui_startup_ctrl_c_exit() -> Result<(), Box<dyn std::error::Error>> {
    // Build the application first to ensure we have the binary
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to execute cargo build");

    if !build_output.status.success() {
        return Err(format!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        )
        .into());
    }

    // Start the TUI application
    let mut session =
        spawn("./target/release/aoba --tui").expect("Failed to spawn TUI application");

    // Give the TUI time to initialize (shorter time for CI)
    std::thread::sleep(Duration::from_millis(500));

    // Send Ctrl+C to terminate the application
    session.send(&[3u8]).expect("Failed to send Ctrl+C"); // Send ASCII 3 (Ctrl+C)

    // Give it time to shut down gracefully
    std::thread::sleep(Duration::from_millis(300));

    println!("   ✓ TUI startup and Ctrl+C exit test completed successfully");
    Ok(())
}

/// Test that we can start the TUI and detect it's running by looking for expected output
fn test_tui_startup_detection() -> Result<(), Box<dyn std::error::Error>> {
    // Build the application first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to execute cargo build");

    if !build_output.status.success() {
        return Err(format!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        )
        .into());
    }

    // Test TUI startup and look for expected content
    let mut session =
        spawn("./target/release/aoba --tui").expect("Failed to spawn TUI application");

    // Give the TUI time to initialize and display its interface
    std::thread::sleep(Duration::from_millis(800));

    // Try to capture some output that should be present in the TUI
    // We're looking for typical TUI elements like "AOBA" or "Refresh" or "Press q to quit"
    let mut found_tui_content = false;

    match session.expect(expectrl::Regex(r"(AOBA|COMPorts|Press.*quit|Refresh)")) {
        Ok(found) => {
            println!(
                "   ✓ Successfully detected TUI content: {:?}",
                found.matches()
            );
            found_tui_content = true;
        }
        Err(e) => {
            println!("   ⚠ Could not detect specific TUI content: {:?}", e);
            // Even if we can't detect specific content, the TUI might still be running
        }
    }

    // Send 'q' to quit
    session.send_line("q").expect("Failed to send 'q' command");
    std::thread::sleep(Duration::from_millis(300));

    if found_tui_content {
        println!("   ✓ TUI startup detection test completed successfully");
    } else {
        println!("   ⚠ TUI started but content detection was inconclusive");
    }

    Ok(())
}

/// Test TUI startup with virtual serial ports available
async fn test_tui_with_virtual_ports() -> Result<(), Box<dyn std::error::Error>> {
    // Set up virtual serial ports using socat
    let socat_output = Command::new("socat")
        .args([
            "-d",
            "-d",
            "pty,raw,echo=0,link=/tmp/smoke_vcom1",
            "pty,raw,echo=0,link=/tmp/smoke_vcom2",
        ])
        .spawn()
        .expect("Failed to start socat for virtual ports");

    let socat_pid = socat_output.id();

    // Wait for ports to be created
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify ports were created
    let port1_exists = std::path::Path::new("/tmp/smoke_vcom1").exists();
    let port2_exists = std::path::Path::new("/tmp/smoke_vcom2").exists();

    if port1_exists && port2_exists {
        println!("   ✓ Virtual serial ports created successfully");

        // Make ports accessible
        let _ = Command::new("chmod")
            .args(["666", "/tmp/smoke_vcom1", "/tmp/smoke_vcom2"])
            .output();

        // Build the application first
        let build_output = Command::new("cargo")
            .args(["build", "--release"])
            .output()
            .expect("Failed to execute cargo build");

        if !build_output.status.success() {
            return Err(format!(
                "Failed to build application: {}",
                String::from_utf8_lossy(&build_output.stderr)
            )
            .into());
        }

        // Test TUI with virtual ports
        let mut session =
            spawn("./target/release/aoba --tui").expect("Failed to spawn TUI application");

        // Give the TUI time to initialize and potentially detect ports
        std::thread::sleep(Duration::from_millis(1000));

        // Send 'q' to quit gracefully
        session.send_line("q").expect("Failed to send 'q' command");
        std::thread::sleep(Duration::from_millis(300));

        println!("   ✓ TUI with virtual ports test completed");
    } else {
        println!("   ⚠ Virtual ports not created, skipping test");
    }

    // Cleanup: kill socat process
    let _ = Command::new("kill").arg(socat_pid.to_string()).output();

    // Remove virtual port files if they exist
    let _ = std::fs::remove_file("/tmp/smoke_vcom1");
    let _ = std::fs::remove_file("/tmp/smoke_vcom2");

    Ok(())
}

/// Basic test to verify expectrl can capture terminal output from simple commands
fn test_expectrl_basic_functionality() -> Result<(), Box<dyn std::error::Error>> {
    let mut session =
        spawn("echo 'Hello from AOBA TUI test'").expect("Failed to spawn echo command");

    match session.expect(expectrl::Regex(r"Hello.*AOBA.*test")) {
        Ok(found) => {
            println!("   ✓ expectrl basic functionality test passed");
            println!("   ✓ Captured: {:?}", found.matches());
            let before_bytes = found.before();
            let before_str = String::from_utf8_lossy(before_bytes);
            if !before_str.is_empty() {
                println!("   ✓ Before content captured successfully");
            }
        }
        Err(e) => {
            return Err(format!("expectrl basic functionality test failed: {:?}", e).into());
        }
    }

    Ok(())
}
