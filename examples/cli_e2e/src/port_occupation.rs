use anyhow::{anyhow, Result};
#[cfg(not(windows))]
use std::process::Stdio;
use std::{process::Command, thread::sleep, time::Duration};

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
    log::info!("Using test port: {test_port}");

    // Check free port
    log::info!("Checking free port...");
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
        log::warn!("⚠️ Port {test_port} might be occupied by another program");
    } else {
        log::info!("✅ Port {test_port} is free");
    }

    // Occupy port with PowerShell and detect
    log::info!("Occupying port with PowerShell...");
    let ps_script = format!(
        r#"
        $port = New-Object System.IO.Ports.SerialPort('{test_port}', 9600)
        try {{
            $port.Open()
            Start-Sleep -Seconds 5
        }} finally {{
            if ($port.IsOpen) {{ $port.Close() }}
        }}
        "#
    );

    let mut occupy_process = Command::new("powershell")
        .args(["-Command", &ps_script])
        .spawn()?;

    sleep(Duration::from_secs(2));

    // Check while occupied
    log::info!("Checking occupied port...");
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
        log::info!("✅ Port {test_port} correctly detected as OCCUPIED");
    } else {
        log::error!("❌ Port {test_port} NOT detected as occupied (FAILED)");
        occupy_process.kill()?;
        return Err(anyhow!("Windows API detection failed to detect occupation"));
    }

    // Wait for PowerShell to finish
    occupy_process.wait()?;
    sleep(Duration::from_secs(1));

    // Check after release
    log::info!("Checking port after release...");
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
        log::info!("✅ Port {test_port} correctly detected as FREE after release");
    } else {
        log::warn!("⚠️ Port {test_port} still detected as occupied");
    }

    log::info!("✅ Windows port occupation detection test completed");
    Ok(())
}

/// Test port occupation detection on Linux platform
#[cfg(not(windows))]
pub fn test_port_occupation_detection_linux(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Testing port occupation detection on Linux...");

    // Get aoba binary path
    let bin_path = build_debug_bin("aoba")?;

    // Check free ports
    log::info!("Checking free ports...");
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

    // Occupy port1 with slave process
    log::info!("Occupying {} with slave process...", port1);
    let mut slave_process = Command::new(&bin_path)
        .args(["--slave-listen-persist", port1])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(2));
    log::info!("✅ Slave process started (PID: {})", slave_process.id());

    if let Some(status) = slave_process.try_wait()? {
        log::error!(
            "❌ Slave process exited early with status: {} (port may be inaccessible)",
            status
        );
        return Err(anyhow!(
            "Slave listener exited before occupation check; verify permissions for {}",
            port1
        ));
    }

    // Check occupied port
    log::info!("Checking occupied port...");
    let check3 = Command::new(&bin_path)
        .args(["--check-port", port1])
        .status()?;

    if check3.code() == Some(1) {
        log::info!("✅ Port {} correctly detected as OCCUPIED", port1);
    } else {
        log::error!("❌ Port {} NOT detected as occupied (FAILED)", port1);
        slave_process.kill()?;
        slave_process.wait()?;
        return Err(anyhow!("Linux detection failed to detect occupation"));
    }

    // Check port2 is still free
    log::info!("Verifying {} is still free...", port2);
    let check4 = Command::new(&bin_path)
        .args(["--check-port", port2])
        .status()?;

    if check4.code() == Some(0) {
        log::info!("✅ Port {} is still FREE", port2);
    } else {
        log::error!("❌ Port {} incorrectly detected as occupied", port2);
    }

    // Release and check
    log::info!("Releasing port and checking...");
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

    log::info!("✅ Linux port occupation detection test completed");
    Ok(())
}
