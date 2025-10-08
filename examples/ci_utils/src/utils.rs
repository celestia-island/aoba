use anyhow::Result;
use std::process::Command;

/// Factory function to create a Modbus command. This mirrors the helper used
/// by the cli_e2e example so tests can spawn the `aoba` binary with the
/// appropriate subcommands.
pub fn create_modbus_command(
    is_slave: bool,
    port: &str,
    is_persist: bool,
    output_or_source: Option<&str>,
) -> Result<Command> {
    let binary = crate::terminal::build_debug_bin("aoba")?;
    let mode = if is_persist { "-persist" } else { "" };
    let mut args: Vec<String> = vec![
        format!(
            "--{}{}",
            if is_slave {
                "slave-listen"
            } else {
                "master-provide"
            },
            mode
        ),
        port.to_string(),
        "--station-id".to_string(),
        "1".to_string(),
        "--register-address".to_string(),
        "0".to_string(),
        "--register-length".to_string(),
        "5".to_string(),
        "--register-mode".to_string(),
        "holding".to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
    ];

    if let Some(out_src) = output_or_source {
        if is_slave {
            args.push("--output".to_string());
        } else {
            args.push("--data-source".to_string());
        }
        args.push(out_src.to_string());
    }

    let mut cmd = Command::new(binary);
    cmd.args(args.iter());
    Ok(cmd)
}
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
