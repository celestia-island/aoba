// CLI integration tests for basic functionality
// This is a dedicated example for testing CLI functionality, not for production release
// Run with: cargo run --example cli_integration_tests

use anyhow::{anyhow, Result};
use std::process::Command;

fn main() -> Result<()> {
    // Initialize logger so CI can capture structured output. Honor RUST_LOG env var.
    let _ = env_logger::try_init();

    log::info!("🔧 Starting CLI Integration Tests...");

    // Build the application first to ensure we have the binary
    log::info!("Building application...");
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .map_err(|err| anyhow!("Failed to execute cargo build: {}", err))?;

    // Log raw build stdout/stderr for CI visibility
    log::debug!(
        "build stdout: {}",
        String::from_utf8_lossy(&build_output.stdout)
    );
    log::debug!(
        "build stderr: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    if !build_output.status.success() {
        log::error!(
            "Failed to build application: status={}",
            build_output.status
        );
        return Err(anyhow!(
            "Failed to build application: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    // Test 1: CLI help command
    log::info!("✅ Test 1: CLI help command");
    test_cli_help()?;

    // Test 2: CLI list ports
    log::info!("✅ Test 2: CLI list ports command");
    test_cli_list_ports()?;

    // Test 3: CLI list ports with JSON output
    log::info!("✅ Test 3: CLI list ports with JSON output");
    test_cli_list_ports_json()?;

    log::info!("🎉 All CLI integration tests passed!");
    Ok(())
}

/// Test CLI help command functionality
fn test_cli_help() -> Result<()> {
    let output = Command::new("./target/release/aoba")
        .arg("--help")
        .output()
        .map_err(|err| anyhow!("Failed to execute aoba binary: {}", err))?;

    // Log raw stdout/stderr for debugging in CI
    log::debug!("help stdout: {}", String::from_utf8_lossy(&output.stdout));
    log::debug!("help stderr: {}", String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        log::error!("Help command failed with status: {}", output.status);
        return Err(anyhow!(
            "Help command failed with status: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("Usage: aoba") {
        log::error!("Help output doesn't contain expected usage text");
        return Err(anyhow!("Help output doesn't contain expected usage text"));
    }

    log::info!("   ✓ Help command works correctly");
    Ok(())
}

/// Test CLI list ports command
fn test_cli_list_ports() -> Result<()> {
    let output = Command::new("./target/release/aoba")
        .arg("--list-ports")
        .output()
        .map_err(|err| anyhow!("Failed to execute aoba binary: {}", err))?;

    // Log raw stdout/stderr for CI visibility
    log::debug!(
        "list-ports stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    log::debug!(
        "list-ports stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        log::error!("List ports command failed with status: {}", output.status);
        return Err(anyhow!(
            "List ports command failed with status: {}",
            output.status
        ));
    }

    log::info!("   ✓ List ports command works correctly");
    // Flexible check: ensure some serial ports are available (CI environment has ttyS devices)
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("/dev/ttyV1") || !stdout.contains("/dev/ttyV2") {
        log::error!(
            "Expected /dev/ttyV1 and /dev/ttyV2 to be present in list-ports output; got: {}",
            stdout
        );
        return Err(anyhow!(
            "Virtual serial ports not found in list-ports output"
        ));
    }

    log::info!(
        "   ✓ Found {} serial ports available for testing",
        port_count
    );
    Ok(())
}

/// Test CLI list ports with JSON output
fn test_cli_list_ports_json() -> Result<()> {
    let output = Command::new("./target/release/aoba")
        .arg("--list-ports")
        .arg("--json")
        .output()
        .map_err(|err| anyhow!("Failed to execute aoba binary: {}", err))?;

    // Log raw stdout/stderr for CI visibility
    log::debug!(
        "list-ports-json stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    log::debug!(
        "list-ports-json stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        log::error!(
            "List ports JSON command failed with status: {}",
            output.status
        );
        return Err(anyhow!(
            "List ports JSON command failed with status: {}",
            output.status
        ));
    }

    log::info!("   ✓ JSON output command works correctly");
    // Flexible check for JSON output: ensure sufficient serial ports are available
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("/dev/ttyV1") || !stdout.contains("/dev/ttyV2") {
        log::error!(
            "Expected /dev/ttyV1 and /dev/ttyV2 in JSON list-ports output; got: {}",
            stdout
        );
        return Err(anyhow!(
            "Virtual serial ports not found in JSON list-ports output"
        ));
    }

    log::info!(
        "   ✓ Found {} serial ports available for JSON testing",
        port_count
    );
    Ok(())
}
