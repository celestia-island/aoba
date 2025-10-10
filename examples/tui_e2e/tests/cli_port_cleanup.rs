use anyhow::{anyhow, Result};
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use ci_utils::{ports::vcom_matchers, terminal::build_debug_bin};

/// Test that CLI programs correctly release ports when they exit
/// This test verifies that:
/// 1. CLI can open a port
/// 2. CLI releases the port on exit
/// 3. Another CLI process can immediately open the same port
pub async fn test_cli_port_release() -> Result<()> {
    // Temporarily set log level to debug for this test
    std::env::set_var("RUST_LOG", "debug");
    
    log::info!("🧪 Starting CLI port release test");

    let ports = vcom_matchers();
    let binary = build_debug_bin("aoba")?;

    // Verify ports exist
    if !std::path::Path::new(&ports.port1_name).exists() {
        return Err(anyhow!("{} does not exist", ports.port1_name));
    }

    log::info!("🧪 Step 1: Start first CLI process to occupy port");
    
    // Start a CLI process that will listen on the port
    let mut child1 = Command::new(&binary)
        .args([
            "--slave-listen-persist",
            &ports.port1_name,
            "--station-id",
            "1",
            "--baud-rate",
            "9600",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give it time to open the port
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Check if it's still running
    if let Ok(Some(status)) = child1.try_wait() {
        let stderr = if let Some(stderr) = child1.stderr {
            std::io::read_to_string(stderr).unwrap_or_default()
        } else {
            String::new()
        };
        return Err(anyhow!(
            "First CLI process exited early with status: {:?}, stderr: {}",
            status,
            stderr
        ));
    }

    log::info!("✅ First CLI process is running and has opened the port");

    log::info!("🧪 Step 2: Send SIGTERM to first CLI process (allows cleanup)");
    
    // Send SIGTERM instead of SIGKILL to allow cleanup handlers to run
    #[cfg(unix)]
    {
        let pid = child1.id();
        let output = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .output()?;
        
        if !output.status.success() {
            log::warn!("Failed to send SIGTERM, falling back to SIGKILL");
            child1.kill()?;
        } else {
            log::info!("Sent SIGTERM to process {}", pid);
        }
    }
    
    #[cfg(not(unix))]
    {
        // On non-Unix, fall back to kill (SIGKILL)
        child1.kill()?;
    }
    
    // Wait for process to fully exit
    let exit_status = child1.wait()?;
    log::info!("✅ First CLI process exited with status: {:?}", exit_status);

    // Give OS more time to release the port - serialport cleanup can take time
    // After testing, found that OS needs significant time to release the FD
    // and reset the port state
    log::info!("Waiting for port to be fully released and reset...");
    tokio::time::sleep(Duration::from_millis(3000)).await;
    
    // Debug: Check if port is still locked
    let lsof_output = Command::new("sudo")
        .args(["lsof", &ports.port1_name])
        .output();
    if let Ok(output) = lsof_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            log::warn!("⚠️ Port {} is still in use:\n{}", ports.port1_name, stdout);
        } else {
            log::info!("✅ Port {} appears to be free according to lsof", ports.port1_name);
        }
    }

    log::info!("🧪 Step 3: Try to open the same port with a second CLI process");
    
    // Try to start another CLI process on the same port
    let mut child2 = Command::new(&binary)
        .args([
            "--slave-listen-persist",
            &ports.port1_name,
            "--station-id",
            "1",
            "--baud-rate",
            "9600",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give it time to open the port
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Check if it's running successfully
    match child2.try_wait()? {
        Some(status) => {
            let stderr = if let Some(mut stderr) = child2.stderr.take() {
                use std::io::Read;
                let mut s = String::new();
                stderr.read_to_string(&mut s).unwrap_or_default();
                s
            } else {
                String::new()
            };
            
            // Clean up
            let _ = child2.kill();
            
            return Err(anyhow!(
                "Second CLI process failed to start or exited early with status: {:?}\nStderr: {}\n\
                This suggests the port was not properly released by the first process.",
                status,
                stderr
            ));
        }
        None => {
            log::info!("✅ Second CLI process is running - port was successfully released!");
            
            // Clean up the second process
            child2.kill()?;
            child2.wait()?;
        }
    }

    log::info!("🧪 Step 4: Verify port can be opened a third time (extra verification)");
    
    // One more test to be sure
    let mut child3 = Command::new(&binary)
        .args([
            "--slave-listen-persist",
            &ports.port1_name,
            "--station-id",
            "1",
            "--baud-rate",
            "9600",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Ok(Some(status)) = child3.try_wait() {
        let stderr = if let Some(mut stderr) = child3.stderr.take() {
            use std::io::Read;
            let mut s = String::new();
            stderr.read_to_string(&mut s).unwrap_or_default();
            s
        } else {
            String::new()
        };
        
        let _ = child3.kill();
        
        return Err(anyhow!(
            "Third CLI process failed with status: {:?}\nStderr: {}",
            status,
            stderr
        ));
    }

    log::info!("✅ Third CLI process is running - port release is working correctly!");
    
    // Final cleanup
    child3.kill()?;
    child3.wait()?;

    log::info!("✅ CLI port release test passed - ports are being released correctly");
    Ok(())
}
