use regex::Regex;

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
pub fn vcom_matchers() -> VcomMatchers {
    use std::path::Path;

    #[cfg(not(windows))]
    fn extend_unique(target: &mut Vec<String>, extras: impl IntoIterator<Item = String>) {
        for item in extras {
            if !target.iter().any(|existing| existing == &item) {
                target.push(item);
            }
        }
    }

    fn collect_aliases(original: &str) -> Vec<String> {
        let mut aliases: Vec<String> = Vec::new();
        let mut push_unique = |candidate: String| {
            if !aliases.iter().any(|existing| existing == &candidate) {
                aliases.push(candidate);
            }
        };

        push_unique(original.to_string());

        let path = Path::new(original);
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            push_unique(name.to_string());
        }

        if let Ok(canonical) = std::fs::canonicalize(path) {
            let canonical_str = canonical.to_string_lossy().to_string();
            push_unique(canonical_str.clone());
            if let Some(name) = canonical.file_name().and_then(|s| s.to_str()) {
                push_unique(name.to_string());
            }
        }

        if let Ok(target) = std::fs::read_link(path) {
            let mut target_path = target.clone();
            if !target_path.is_absolute() {
                if let Some(parent) = path.parent() {
                    target_path = parent.join(target_path);
                }
            }
            let target_str = target_path.to_string_lossy().to_string();
            push_unique(target_str.clone());
            if let Some(name) = target_path.file_name().and_then(|s| s.to_str()) {
                push_unique(name.to_string());
            }
        }

        aliases
    }

    fn build_pattern(aliases: &[String]) -> String {
        if aliases.is_empty() {
            return String::new();
        }
        let escaped = aliases
            .iter()
            .map(|alias| regex::escape(alias))
            .collect::<Vec<_>>()
            .join("|");
        format!("(?:{escaped})")
    }

    let env1 = std::env::var("AOBATEST_PORT1").ok();
    let env2 = std::env::var("AOBATEST_PORT2").ok();

    let (p1, p2) = if let (Some(a), Some(b)) = (env1, env2) {
        (a, b)
    } else {
        (DEFAULT_PORT1.to_string(), DEFAULT_PORT2.to_string())
    };

    #[cfg(not(windows))]
    let port1_aliases = {
        let mut aliases = collect_aliases(&p1);
        extend_unique(&mut aliases, collect_aliases(DEFAULT_PORT1));
        aliases
    };

    #[cfg(windows)]
    let port1_aliases = collect_aliases(&p1);

    #[cfg(not(windows))]
    let port2_aliases = {
        let mut aliases = collect_aliases(&p2);
        extend_unique(&mut aliases, collect_aliases(DEFAULT_PORT2));
        aliases
    };

    #[cfg(windows)]
    let port2_aliases = collect_aliases(&p2);

    let (port1_rx, port2_rx, cursor_rx) = if cfg!(windows) {
        let p1_e = build_pattern(&port1_aliases);
        let p2_e = build_pattern(&port2_aliases);
        let port1_rx = Regex::new(&format!(r"(?i)\b{p1_e}\b")).unwrap();
        let port2_rx = Regex::new(&format!(r"(?i)\b{p2_e}\b")).unwrap();
        let cursor_rx = Regex::new(&format!(r"(?i)> ?(?:{p1_e}|{p2_e})\b")).unwrap();
        (port1_rx, port2_rx, cursor_rx)
    } else {
        let p1_e = build_pattern(&port1_aliases);
        let p2_e = build_pattern(&port2_aliases);
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
        port1_aliases,
        port2_aliases,
    }
}

/// Check if a serial port exists on the current platform
pub fn port_exists(port_name: &str) -> bool {
    #[cfg(windows)]
    {
        // On Windows, try to list available ports and check if our port is in the list
        // This is more reliable than trying to open the port directly
        if let Ok(ports) = serialport::available_ports() {
            return ports
                .iter()
                .any(|p| p.port_name.eq_ignore_ascii_case(port_name));
        }
        // If we can't list ports, assume the port exists (fail later if it doesn't work)
        log::warn!("Could not list serial ports on Windows, assuming {port_name} exists");
        true
    }

    #[cfg(not(windows))]
    {
        // On Unix-like systems, check if the device file exists
        std::path::Path::new(port_name).exists()
    }
}

/// Decide whether virtual serial port (vcom) tests should run on this platform.
pub fn should_run_vcom_tests() -> bool {
    // Allow explicit override via environment variable
    if let Ok(val) = std::env::var("CI_FORCE_VCOM") {
        let should_run = val == "1" || val.eq_ignore_ascii_case("true");
        log::info!("CI_FORCE_VCOM={val}, should_run={should_run}");
        return should_run;
    }

    // On Windows, check if the test ports are available
    #[cfg(windows)]
    {
        let ports = vcom_matchers();
        log::info!(
            "Checking for ports: {} and {}",
            ports.port1_name,
            ports.port2_name
        );

        let port1_exists = port_exists(&ports.port1_name);
        let port2_exists = port_exists(&ports.port2_name);

        log::info!(
            "Port existence check: {} exists={}, {} exists={}",
            ports.port1_name,
            port1_exists,
            ports.port2_name,
            port2_exists
        );

        if !port1_exists || !port2_exists {
            log::info!(
                "Virtual serial port tests disabled on Windows: {} exists={}, {} exists={}",
                ports.port1_name,
                port1_exists,
                ports.port2_name,
                port2_exists
            );
            return false;
        }
        log::info!("Both ports available, tests will run");
        true
    }

    // On Unix-like systems, always run tests (socat creates ports on demand)
    #[cfg(not(windows))]
    {
        true
    }
}
