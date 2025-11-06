use anyhow::{anyhow, Result};
use regex::Regex;
use std::{path::PathBuf, process::Command, time::Duration};

/// Platform-specific default port names as constants
#[cfg(windows)]
pub const DEFAULT_PORT1: &str = "COM1";
#[cfg(windows)]
pub const DEFAULT_PORT2: &str = "COM2";

#[cfg(not(windows))]
pub const DEFAULT_PORT1: &str = "/tmp/vcom1";
#[cfg(not(windows))]
pub const DEFAULT_PORT2: &str = "/tmp/vcom2";

/// Helper struct describing the regexes and display names used to detect
/// the two expected virtual serial ports in TUI output.
#[allow(dead_code)]
pub struct VcomMatchers {
    pub port1_rx: Regex,
    pub port2_rx: Regex,
    pub cursor_rx: Regex,
    pub port1_name: String,
    pub port2_name: String,
    pub port1_aliases: Vec<String>,
    pub port2_aliases: Vec<String>,
}

/// Build platform-appropriate Regex matchers for the two virtual ports.
pub fn vcom_matchers_with_ports(port1: &str, port2: &str) -> VcomMatchers {
    let port1_pattern = regex::escape(port1);
    let port2_pattern = regex::escape(port2);

    VcomMatchers {
        port1_rx: Regex::new(&port1_pattern).unwrap(),
        port2_rx: Regex::new(&port2_pattern).unwrap(),
        cursor_rx: Regex::new(r">").unwrap(),
        port1_name: port1.to_string(),
        port2_name: port2.to_string(),
        port1_aliases: vec![port1.to_string()],
        port2_aliases: vec![port2.to_string()],
    }
}

/// Locate the project's debug binary for a specific bin name and return the path to the executable.
pub fn build_debug_bin(bin_name: &str) -> Result<PathBuf> {
    let workspace_root = std::env::current_dir()?
        .ancestors()
        .find(|p| {
            let cargo_toml = p.join("Cargo.toml");
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                content.contains("[workspace]") || content.contains("name = \"aoba\"")
            } else {
                false
            }
        })
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("Could not find workspace root"))?;

    log::info!("🔍 Workspace root: {path}", path = workspace_root.display());

    let exe_name = if cfg!(windows) {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    };

    let bin_paths = [
        workspace_root
            .join("target")
            .join("release")
            .join(&exe_name),
        workspace_root.join("target").join("debug").join(&exe_name),
    ];

    let bin_path = bin_paths
        .iter()
        .find(|p| p.exists())
        .ok_or_else(|| {
            anyhow!(
                "Binary not found at any of: {}. Run `cargo build --bin {}` first.",
                bin_paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                bin_name
            )
        })?
        .to_path_buf();

    log::info!("✅ Using prebuilt binary: {}", bin_path.display());
    Ok(bin_path)
}

/// Run the aoba binary synchronously with the given arguments
pub fn run_binary_sync(args: &[&str]) -> Result<std::process::Output> {
    let bin_path = build_debug_bin("aoba")?;

    log::info!("▶️ Running binary: {} {:?}", bin_path.display(), args);
    let output = Command::new(&bin_path)
        .args(args)
        .output()
        .map_err(|err| anyhow!("Failed to execute binary {}: {}", bin_path.display(), err))?;

    Ok(output)
}

/// Sleep for 1 second (1000ms) - standard delay for CI/E2E tests
pub async fn sleep_1s() {
    tokio::time::sleep(Duration::from_millis(1000)).await;
}

/// Check if a serial port exists
#[allow(dead_code)]
pub fn port_exists(port_name: &str) -> bool {
    #[cfg(windows)]
    {
        // On Windows, try to list available ports and check if our port is in the list
        if let Ok(ports) = serialport::available_ports() {
            return ports
                .iter()
                .any(|p| p.port_name.eq_ignore_ascii_case(port_name));
        }
        log::warn!("Could not list serial ports on Windows, assuming {port_name} exists");
        true
    }

    #[cfg(not(windows))]
    {
        // On Unix-like systems, check if the device file exists
        std::path::Path::new(port_name).exists()
    }
}

/// Check if VCOM tests should run with the specified ports
pub fn should_run_vcom_tests_with_ports(port1: &str, port2: &str) -> bool {
    // Silence unused-variable warnings on non-Windows platforms where these
    // parameters are not referenced by the function body.
    let _port1 = port1;
    let _port2 = port2;
    // Allow explicit override via environment variable
    if let Ok(val) = std::env::var("CI_FORCE_VCOM") {
        let should_run = val == "1" || val.eq_ignore_ascii_case("true");
        log::info!("CI_FORCE_VCOM={val}, should_run={should_run}");
        return should_run;
    }

    // On Windows, check if the test ports are available
    #[cfg(windows)]
    {
        log::info!("Checking for ports: {port1} and {port2}");

        let port1_exists = port_exists(port1);
        let port2_exists = port_exists(port2);

        log::info!(
            "Port existence check: {port1} exists={port1_exists}, {port2} exists={port2_exists}"
        );

        if !port1_exists || !port2_exists {
            log::info!("Virtual serial port tests disabled on Windows: missing ports");
            return false;
        }
        log::info!("Both ports available, tests will run");
        true
    }

    // On Unix-like systems, always run tests (socat creates ports on demand)
    #[cfg(not(windows))]
    {
        log::info!("Unix-like system detected, running VCOM tests");
        true
    }
}

/// Factory function to create a Modbus command
///
/// # Arguments
/// * `is_slave` - true for slave (server), false for master (client)
/// * `port` - Serial port path (e.g., "/tmp/vcom1")
/// * `is_persist` - true for persistent mode, false for temporary mode
/// * `output_or_source` - Optional output file for slave or data source file for master
///
/// # Returns
/// A Command that can be further configured and executed
#[allow(dead_code)]
pub fn create_modbus_command(
    is_slave: bool,
    port: &str,
    is_persist: bool,
    output_or_source: Option<&str>,
) -> Result<Command> {
    let binary = crate::utils::build_debug_bin("aoba")?;
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
