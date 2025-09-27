// CLI integration tests for basic functionality
// This is a dedicated example for testing CLI functionality, not for production release
// Run with: cargo run --example cli_integration_tests

use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Starting CLI Integration Tests...");

    // Build the application first to ensure we have the binary
    println!("Building application...");
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

    // Test 1: CLI help command
    println!("✅ Test 1: CLI help command");
    test_cli_help()?;

    // Test 2: CLI list ports
    println!("✅ Test 2: CLI list ports command");
    test_cli_list_ports()?;

    // Test 3: CLI list ports with JSON output
    println!("✅ Test 3: CLI list ports with JSON output");
    test_cli_list_ports_json()?;

    println!("🎉 All CLI integration tests passed!");
    Ok(())
}

/// Test CLI help command functionality
fn test_cli_help() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("./target/release/aoba")
        .arg("--help")
        .output()
        .map_err(|e| format!("Failed to execute aoba binary: {}", e))?;

    if !output.status.success() {
        return Err(format!("Help command failed with status: {}", output.status).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("Usage: aoba") {
        return Err("Help output doesn't contain expected usage text".into());
    }

    println!("   ✓ Help command works correctly");
    Ok(())
}

/// Test CLI list ports command
fn test_cli_list_ports() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("./target/release/aoba")
        .arg("--list-ports")
        .output()
        .map_err(|e| format!("Failed to execute aoba binary: {}", e))?;

    if !output.status.success() {
        return Err(format!("List ports command failed with status: {}", output.status).into());
    }

    println!("   ✓ List ports command works correctly");
    Ok(())
}

/// Test CLI list ports with JSON output
fn test_cli_list_ports_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("./target/release/aoba")
        .arg("--list-ports")
        .arg("--json")
        .output()
        .map_err(|e| format!("Failed to execute aoba binary: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "List ports JSON command failed with status: {}",
            output.status
        )
        .into());
    }

    println!("   ✓ JSON output command works correctly");
    Ok(())
}
