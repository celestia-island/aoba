use anyhow::{anyhow, Result};
use std::{process::Command, thread::sleep, time::Duration};

#[cfg(not(windows))]
use std::process::Stdio;

#[cfg(not(windows))]
use crate::utils::build_debug_bin;

/// Test port occupation detection on Windows platform
#[cfg(windows)]
pub fn test_port_occupation_detection_windows() -> Result<()> {
    log::info!("🧪 Testing port occupation detection on Windows...");

    // Get available COM ports
    let ports_output = Command::new("cargo")
        .args(["run", "--package", "aoba", "--", "--list-ports"])
        .output()?;

    if !ports_output.status.success() {
        return Err(anyhow!("Failed to list COM ports"));
    }

    let ports_str = String::from_utf8_lossy(&ports_output.stdout);
    let ports: Vec<&str> = ports_str.lines().filter(|line| !line.is_empty()).collect();

    if ports.is_empty() {
        log::warn!("⚠️ No COM ports found on Windows, skipping test");
        return Ok(());
    }

    let test_port = ports[0];
    log::info!("Using test port: {}", test_port);

    // Test 1: Check free port
    log::info!("Test 1: Checking free port...");
    let check1 = Command::new("cargo")
        .args([
            "run",
            "--package",
            "aoba",
            "--quiet",
            "--",
            "--check-port",
            test_port,
        ])
        .status()?;

    if check1.code() != Some(0) {
        log::warn!("⚠️ Port {} might be occupied by another program", test_port);
    } else {
        log::info!("✅ Port {} is free", test_port);
    }

    // Test 2: Occupy port with PowerShell and detect
    log::info!("Test 2: Occupying port with PowerShell...");
    let ps_script = format!(
        r#"
        $port = New-Object System.IO.Ports.SerialPort('{}', 9600)
        try {{
            $port.Open()
            Start-Sleep -Seconds 5
        }} finally {{
            if ($port.IsOpen) {{ $port.Close() }}
        }}
        "#,
        test_port
    );

    let mut occupy_process = Command::new("powershell")
        .args(["-Command", &ps_script])
        .spawn()?;

    sleep(Duration::from_secs(2));

    // Check while occupied
    log::info!("Test 3: Checking occupied port...");
    let check2 = Command::new("cargo")
        .args([
            "run",
            "--package",
            "aoba",
            "--quiet",
            "--",
            "--check-port",
            test_port,
        ])
        .status()?;

    if check2.code() == Some(1) {
        log::info!("✅ Port {} correctly detected as OCCUPIED", test_port);
    } else {
        log::error!("❌ Port {} NOT detected as occupied (FAILED)", test_port);
        occupy_process.kill()?;
        return Err(anyhow!("Windows API detection failed to detect occupation"));
    }

    // Wait for PowerShell to finish
    occupy_process.wait()?;
    sleep(Duration::from_secs(1));

    // Test 4: Check after release
    log::info!("Test 4: Checking port after release...");
    let check3 = Command::new("cargo")
        .args([
            "run",
            "--package",
            "aoba",
            "--quiet",
            "--",
            "--check-port",
            test_port,
        ])
        .status()?;

    if check3.code() == Some(0) {
        log::info!(
            "✅ Port {} correctly detected as FREE after release",
            test_port
        );
    } else {
        log::warn!("⚠️ Port {} still detected as occupied", test_port);
    }

    log::info!("✅ Windows port occupation detection test completed");
    Ok(())
}

/// Test port occupation detection on Linux platform
#[cfg(not(windows))]
pub fn test_port_occupation_detection_linux() -> Result<()> {
    log::info!("🧪 Testing port occupation detection on Linux...");

    let port1 = "/tmp/vcom1";
    let port2 = "/tmp/vcom2";

    // Get aoba binary path
    let bin_path = build_debug_bin("aoba")?;

    // Test 1: Check free ports
    log::info!("Test 1: Checking free ports...");
    let check1 = Command::new(&bin_path)
        .args(["--check-port", port1])
        .status()?;

    if check1.code() == Some(0) {
        log::info!("✅ Port {} is FREE", port1);
    } else {
        log::warn!("⚠️ Port {} detected as occupied", port1);
    }

    let check2 = Command::new(&bin_path)
        .args(["--check-port", port2])
        .status()?;

    if check2.code() == Some(0) {
        log::info!("✅ Port {} is FREE", port2);
    } else {
        log::warn!("⚠️ Port {} detected as occupied", port2);
    }

    // Test 2: Occupy port1 with slave process
    log::info!("Test 2: Occupying {} with slave process...", port1);
    let mut slave_process = Command::new(&bin_path)
        .args(["--slave-listen-persist", port1])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(2));
    log::info!("✅ Slave process started (PID: {})", slave_process.id());

    // Test 3: Check occupied port
    log::info!("Test 3: Checking occupied port...");
    let check3 = Command::new(&bin_path)
        .args(["--check-port", port1])
        .status()?;

    if check3.code() == Some(1) {
        log::info!("✅ Port {} correctly detected as OCCUPIED", port1);
    } else {
        log::error!("❌ Port {} NOT detected as occupied (FAILED)", port1);
        slave_process.kill()?;
        cleanup_linux_ports()?;
        return Err(anyhow!("Linux detection failed to detect occupation"));
    }

    // Test 4: Check port2 is still free
    log::info!("Test 4: Verifying {} is still free...", port2);
    let check4 = Command::new(&bin_path)
        .args(["--check-port", port2])
        .status()?;

    if check4.code() == Some(0) {
        log::info!("✅ Port {} is still FREE", port2);
    } else {
        log::error!("❌ Port {} incorrectly detected as occupied", port2);
    }

    // Test 5: Release and check
    log::info!("Test 5: Releasing port and checking...");
    slave_process.kill()?;
    slave_process.wait()?;
    sleep(Duration::from_secs(3));

    let check5 = Command::new(&bin_path)
        .args(["--check-port", port1])
        .status()?;

    if check5.code() == Some(0) {
        log::info!("✅ Port {} detected as FREE after release", port1);
    } else {
        log::warn!(
            "⚠️ Port {} still shows as occupied (may need more time)",
            port1
        );
    }

    // Cleanup
    cleanup_linux_ports()?;

    log::info!("✅ Linux port occupation detection test completed");
    Ok(())
}

#[cfg(not(windows))]
fn cleanup_linux_ports() -> Result<()> {
    log::info!("Cleaning up virtual serial ports...");
    let _ = Command::new("pkill")
        .args(["-9", "-f", "socat.*pty.*vcom"])
        .status();
    let _ = Command::new("rm")
        .args(["-f", "/tmp/vcom1", "/tmp/vcom2"])
        .status();
    Ok(())
}
