use regex::Regex;

/// Helper struct describing the regexes and display names used to detect
/// the two expected virtual serial ports in TUI output.
pub struct VcomMatchers {
    pub port1_rx: Regex,
    pub port2_rx: Regex,
    pub cursor_rx: Regex,
    pub port1_name: String,
    pub port2_name: String,
}

/// Build platform-appropriate Regex matchers for the two virtual ports.
pub fn vcom_matchers() -> VcomMatchers {
    let env1 = std::env::var("AOBATEST_PORT1").ok();
    let env2 = std::env::var("AOBATEST_PORT2").ok();

    let (p1, p2) = if let (Some(a), Some(b)) = (env1, env2) {
        (a, b)
    } else if cfg!(windows) {
        ("COM1".to_string(), "COM2".to_string())
    } else {
        ("/tmp/vcom1".to_string(), "/tmp/vcom2".to_string())
    };

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
pub fn should_run_vcom_tests() -> bool {
    if !cfg!(unix) {
        return std::env::var("CI_FORCE_VCOM")
            .map(|v| v == "1")
            .unwrap_or(false);
    }
    true
}
