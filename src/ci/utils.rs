use regex::Regex;
use std::time::Duration;

/// Helper struct describing the regexes and display names used to detect
/// the two expected virtual serial ports in TUI output.
pub struct VcomMatchers {
    /// Regex that matches the first virtual port in generic output
    pub port1_rx: Regex,
    /// Regex that matches the second virtual port in generic output
    pub port2_rx: Regex,
    /// Regex that matches the cursor marker when it points at either port
    pub cursor_rx: Regex,
    /// Human-friendly name for port1 (e.g. "COM1" or "/dev/vcom1")
    pub port1_name: String,
    /// Human-friendly name for port2 (e.g. "COM2" or "/dev/vcom2")
    pub port2_name: String,
}

/// Build platform-appropriate Regex matchers for the two virtual ports.
///
/// Behavior:
/// - If env vars `AOBATEST_PORT1` and `AOBATEST_PORT2` are set, their values
///   are used as the expected display names.
/// - Otherwise on Windows the defaults are `COM1`/`COM2`; on other platforms
///   the defaults are `/dev/vcom1` and `/dev/vcom2`.
///
/// The function returns a ready-to-use `VcomMatchers` with compiled Regexes.
pub fn vcom_matchers() -> VcomMatchers {
    let env1 = std::env::var("AOBATEST_PORT1").ok();
    let env2 = std::env::var("AOBATEST_PORT2").ok();

    let (p1, p2) = if let (Some(a), Some(b)) = (env1, env2) {
        (a, b)
    } else if cfg!(windows) {
        ("COM1".to_string(), "COM2".to_string())
    } else {
        ("/dev/vcom1".to_string(), "/dev/vcom2".to_string())
    };

    // Build Regexes. For Windows we use word boundaries and case-insensitive
    // matching to match names like "COM1". For Unix-like names (which may
    // include slashes) we match the literal escaped string.
    let (port1_rx, port2_rx, cursor_rx) = if cfg!(windows) {
        let p1_e = regex::escape(&p1);
        let p2_e = regex::escape(&p2);
        let port1_rx = Regex::new(&format!(r"(?i)\b{p1_e}\b")).unwrap();
        let port2_rx = Regex::new(&format!(r"(?i)\b{p2_e}\b")).unwrap();
        let cursor_rx = Regex::new(&format!(r"(?i)> ?(?:{p1_e}|{p2_e})\b")).unwrap();
        (port1_rx, port2_rx, cursor_rx)
    } else {
        let p1_e = regex::escape(&p1);
        let p2_e = regex::escape(&p2);
        let port1_rx = Regex::new(&p1_e).unwrap();
        let port2_rx = Regex::new(&p2_e).unwrap();
        let cursor_rx = Regex::new(&format!(r"> ?(?:{p1_e}|{p2_e})")).unwrap();
        (port1_rx, port2_rx, cursor_rx)
    };

    VcomMatchers {
        port1_rx,
        port2_rx,
        cursor_rx,
        port1_name: p1,
        port2_name: p2,
    }
}

/// Decide whether virtual serial port (vcom) tests should run on this platform.
///
/// On non-Unix platforms this will honor the `CI_FORCE_VCOM=1` env var; on
/// Unix it returns true by default.
pub fn should_run_vcom_tests() -> bool {
    if !cfg!(unix) {
        return std::env::var("CI_FORCE_VCOM")
            .map(|v| v == "1")
            .unwrap_or(false);
    }
    true
}

/// A small async helper to make test delays readable and reusable.
///
/// This helper waits for a short, fixed amount of time (500ms). Use it in
/// tests that need a small pause for UI/process responsiveness.
pub async fn sleep_a_while() {
    const DEFAULT_MS: u64 = 500;
    tokio::time::sleep(Duration::from_millis(DEFAULT_MS)).await;
}

/// Reset virtual serial ports by calling the socat_reset.sh script.
///
/// This function should be called between tests to ensure clean state.
/// It kills existing socat processes, removes old symlinks, and creates
/// fresh virtual serial ports.
///
/// Returns Ok(()) if the reset was successful, Err otherwise.
pub fn reset_vcom_ports() -> anyhow::Result<()> {
    if !should_run_vcom_tests() {
        log::debug!("Skipping vcom port reset on this platform");
        return Ok(());
    }

    log::info!("🔄 Resetting virtual serial ports...");
    
    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/tui_e2e_tests/scripts/socat_reset.sh");
    
    if !script_path.exists() {
        return Err(anyhow::anyhow!(
            "socat_reset.sh script not found at {:?}",
            script_path
        ));
    }

    let output = std::process::Command::new("sudo")
        .arg(script_path)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute socat_reset.sh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        log::error!("socat_reset.sh failed with status: {}", output.status);
        log::error!("stdout: {}", stdout);
        log::error!("stderr: {}", stderr);
        return Err(anyhow::anyhow!(
            "socat_reset.sh failed with status: {}",
            output.status
        ));
    }

    log::info!("✓ Virtual serial ports reset successfully");
    Ok(())
}
