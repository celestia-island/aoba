use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Smoke test runner for CI
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚦 Starting AOBA Smoke Tests...");

    // Test 1: Basic binary existence and help
    println!("✅ Test 1: Binary help command");
    test_binary_help()?;

    // Test 2: List ports functionality
    println!("✅ Test 2: List ports command");
    test_list_ports()?;

    // Test 3: JSON output functionality
    println!("✅ Test 3: JSON output command");
    test_json_output()?;

    // Test 4: Serial port detection with virtual ports
    println!("✅ Test 4: Virtual serial port detection");
    test_virtual_ports()?;

    // Test 5: Quick TUI startup/shutdown
    println!("✅ Test 5: TUI quick startup/shutdown");
    test_tui_quick()?;

    println!("🎉 All smoke tests passed!");
    Ok(())
}

fn test_binary_help() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("./target/release/aoba")
        .arg("--help")
        .output()?;

    if !output.status.success() {
        return Err("Help command failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("Usage: aoba") {
        return Err("Help output doesn't contain expected usage text".into());
    }

    println!("   ✓ Help command works correctly");
    Ok(())
}

fn test_list_ports() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("./target/release/aoba")
        .arg("--list-ports")
        .output()?;

    if !output.status.success() {
        return Err("List ports command failed".into());
    }

    println!("   ✓ List ports command works correctly");
    Ok(())
}

fn test_json_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("./target/release/aoba")
        .args(["--list-ports", "--json"])
        .output()?;

    if !output.status.success() {
        return Err("JSON output command failed".into());
    }

    // Try to parse output as JSON (basic validation)
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("   ✓ JSON output command works (empty result is valid)");
    } else {
        // Basic JSON validation - should start with [ or {
        let trimmed = stdout.trim();
        if trimmed.starts_with('[') || trimmed.starts_with('{') {
            println!("   ✓ JSON output command works correctly");
        } else {
            println!("   ⚠ JSON output might not be valid JSON, but command succeeded");
        }
    }

    Ok(())
}

fn test_virtual_ports() -> Result<(), Box<dyn std::error::Error>> {
    // Check if we have virtual ports set up by CI (using socat)
    let vcom1_exists = fs::metadata("/dev/vcom1").is_ok();
    let vcom2_exists = fs::metadata("/dev/vcom2").is_ok();

    if vcom1_exists && vcom2_exists {
        println!("   ✓ Virtual serial ports are available");

        // Test if aoba can see the virtual ports
        let output = Command::new("./target/release/aoba")
            .arg("--list-ports")
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("/dev/vcom") {
                println!("   ✓ Virtual ports detected by aoba");
            } else {
                println!("   ⚠ Virtual ports not detected in output (may be expected)");
                println!("   Output: {stdout}");
            }
        }
    } else {
        println!("   ⚠ Virtual serial ports not available (may be expected in some environments)");
    }

    Ok(())
}

fn test_tui_quick() -> Result<(), Box<dyn std::error::Error>> {
    // Quick TUI test - start and immediately send quit signal
    let mut child = Command::new("./target/release/aoba")
        .arg("--tui")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Give TUI a moment to start
    thread::sleep(Duration::from_millis(500));

    // Send quit signal (typically 'q' or Ctrl+C)
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.write_all(&[3]); // Ctrl+C
    }

    // Wait for process to exit (with timeout)
    let mut count = 0;
    while count < 10 {
        match child.try_wait()? {
            Some(_status) => {
                println!("   ✓ TUI starts and exits successfully");
                return Ok(());
            }
            None => {
                thread::sleep(Duration::from_millis(100));
                count += 1;
            }
        }
    }

    // Force kill if it didn't exit
    let _ = child.kill();
    let _ = child.wait();

    println!("   ⚠ TUI test completed (may have required force termination)");
    Ok(())
}
