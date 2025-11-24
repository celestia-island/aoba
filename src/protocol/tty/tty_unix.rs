use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};

use serialport::{SerialPortInfo, SerialPortType};

use super::{PortExtra, VidPidSerial};

/// Flag that allows manual opt-in to the CI-style virtual port detection logic.
static FORCE_VIRTUAL_PORT_HINT: AtomicBool = AtomicBool::new(false);

/// Enable the virtual port hint without toggling full CI debug dumping.
pub fn enable_virtual_port_hint() {
    FORCE_VIRTUAL_PORT_HINT.store(true, Ordering::SeqCst);
}

fn is_virtual_port_hint_enabled() -> bool {
    FORCE_VIRTUAL_PORT_HINT.load(Ordering::SeqCst)
}

// Utility functions for parsing debug strings (shared with Windows)
fn parse_string_after(s: &str, key: &str) -> Option<String> {
    // Look for key and then accept an '=' separated value; stop at first non-accepted char
    let tail = s.split_once(key)?.1;
    // prefer explicit '=' value first
    if let Some((_, after)) = tail.split_once('=') {
        let after_trim = after.trim_start();
        let mut out = String::new();
        for c in after_trim.chars() {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == ':' || c == '.' {
                out.push(c);
            } else {
                break;
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

fn parse_hex_after(s: &str, key: &str) -> Option<u16> {
    let tail = s.split_once(key)?.1;
    // check for 0x... first
    if let Some((_, after)) = tail.split_once("0x") {
        let num: String = after
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if !num.is_empty() {
            if let Ok(v) = u16::from_str_radix(&num, 16) {
                return Some(v);
            }
        }
    }
    None
}

fn parse_serial_after(s: &str, key: &str) -> Option<String> {
    let tail = s.split_once(key)?.1;
    if let Some((_, after)) = tail.split_once('=') {
        let after_trim = after.trim_start();
        let mut out = String::new();
        for c in after_trim.chars() {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                out.push(c);
            } else {
                break;
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// Detect virtual serial ports created by socat or similar tools
fn detect_virtual_ports() -> Vec<SerialPortInfo> {
    let mut virtual_ports = Vec::new();

    // In CI debug mode (when --debug-ci-e2e-test is set), only detect vcom1 and vcom2
    // to avoid false positives from residual ports (vcom3, vcom4, etc.)
    let virtual_hint = is_virtual_port_hint_enabled();
    let virtual_port_paths =
        if virtual_hint || crate::protocol::status::debug_dump::is_debug_dump_enabled() {
            vec!["/dev/vcom1", "/dev/vcom2", "/tmp/vcom1", "/tmp/vcom2"]
        } else {
            vec![
                "/dev/vcom1",
                "/dev/vcom2",
                "/dev/vcom3",
                "/dev/vcom4",
                "/dev/vcom5",
                "/dev/vcom6",
                "/tmp/vcom1",
                "/tmp/vcom2",
                "/tmp/vcom3",
                "/tmp/vcom4",
                "/tmp/vcom5",
                "/tmp/vcom6",
            ]
        };

    for port_path in &virtual_port_paths {
        if Path::new(port_path).exists() {
            // Check if it's a symlink to a pts device or a character device
            if let Ok(metadata) = fs::symlink_metadata(port_path) {
                // Accept any non-directory filesystem entry here. Some test
                // harnesses create regular files or sockets in /tmp (not
                // strict symlinks/char devices), so treat any existing
                // non-directory path as a virtual port candidate.
                if !metadata.file_type().is_dir() {
                    virtual_ports.push(SerialPortInfo {
                        port_name: port_path.to_string(),
                        port_type: SerialPortType::Unknown,
                    });
                }
            }
        }
    }

    // Fallback for non-sudo environments: dynamically generated temp links (e.g. /tmp/aoba_vcom1.*)
    if let Ok(entries) = fs::read_dir("/tmp") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("aoba_vcom") {
                    if let Ok(metadata) = fs::symlink_metadata(&path) {
                        if !metadata.file_type().is_dir() {
                            virtual_ports.push(SerialPortInfo {
                                port_name: path.to_string_lossy().to_string(),
                                port_type: SerialPortType::Unknown,
                            });
                        }
                    }
                }
            }
        }
    }

    // Legacy fallback: Explicit environment variable overrides (for backward compatibility)
    // NOTE: New code should use command-line arguments (--port1, --port2) instead.
    // This is kept for compatibility with test harness that may set these variables
    // when socat creates ports with non-standard names.
    for env_key in ["AOBATEST_PORT1", "AOBATEST_PORT2"] {
        if let Ok(value) = std::env::var(env_key) {
            let path = Path::new(&value);
            if path.exists() {
                if let Ok(metadata) = fs::symlink_metadata(path) {
                    if !metadata.file_type().is_dir() {
                        virtual_ports.push(SerialPortInfo {
                            port_name: value.clone(),
                            port_type: SerialPortType::Unknown,
                        });
                    }
                }
            }
        }
    }

    virtual_ports
}

fn list_ports_from_shell() -> Vec<String> {
    // Prefer ls over find to keep output simple; we only care about path strings.
    // Use a conservative set of globs to avoid huge directory walks.
    let candidates = [
        "ls /dev/ttyS* /dev/ttyUSB* /dev/ttyACM* 2>/dev/null",
        "ls /dev/tty.* /dev/cu.* 2>/dev/null",
    ];

    for cmd in candidates.iter() {
        let output = Command::new("bash").arg("-lc").arg(cmd).output();
        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut ports = Vec::new();
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        ports.push(trimmed.to_string());
                    }
                }
                if !ports.is_empty() {
                    return ports;
                }
            }
        }
    }

    Vec::new()
}

/// Return the list of available serial ports sorted / deduped for Unix.
pub fn available_ports_sorted() -> Vec<SerialPortInfo> {
    // In CI debug mode (when --debug-ci-e2e-test is set), skip real serial port enumeration
    // and only return virtual ports to avoid interference from host serial devices
    let debug_enabled = crate::protocol::status::debug_dump::is_debug_dump_enabled();
    let mut raw_ports = if debug_enabled || is_virtual_port_hint_enabled() {

        Vec::new()
    } else {
        // Avoid serialport's libudev-based enumeration and instead rely on
        // shell-based detection of common TTY device paths.
        list_ports_from_shell()
            .into_iter()
            .map(|name| SerialPortInfo {
                port_name: name,
                port_type: SerialPortType::Unknown,
            })
            .collect()
    };

    // Add virtual ports created by socat or similar tools
    let virtual_ports = detect_virtual_ports();
    raw_ports.extend(virtual_ports);

    sort_and_dedup_ports(raw_ports)
}

pub fn available_ports_enriched() -> Vec<(SerialPortInfo, PortExtra)> {
    available_ports_sorted()
        .into_iter()
        .map(|p| {
            let meta = try_extract_vid_pid_serial(&p.port_type);
            let (vid, pid, serial, manufacturer, product) = meta
                .map(|(v, p2, s, m, pr)| (Some(v), Some(p2), s, m, pr))
                .unwrap_or((None, None, None, None, None));
            (
                p,
                PortExtra {
                    guid: None,
                    vid,
                    pid,
                    serial,
                    manufacturer,
                    product,
                },
            )
        })
        .collect()
}

pub(crate) fn sort_and_dedup_ports(raw_ports: Vec<SerialPortInfo>) -> Vec<SerialPortInfo> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut unique: Vec<SerialPortInfo> = Vec::new();

    for p in raw_ports.into_iter() {
        let base = match p.port_name.rsplit('/').next() {
            Some(b) => b.to_lowercase(),
            None => p.port_name.to_lowercase(),
        };
        let key = match &p.port_type {
            SerialPortType::UsbPort(info) => {
                format!("{}:vid={:04x}:pid={:04x}", base, info.vid, info.pid)
            }
            _ => base,
        };

        if seen.insert(key) {
            unique.push(p);
        }
    }

    let mut ports = unique;

    // Annotate devices sharing same basename with vid / pid so user can distinguish
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, p) in ports.iter().enumerate() {
        let base = match p.port_name.rsplit('/').next() {
            Some(b) => b.to_lowercase(),
            None => p.port_name.to_lowercase(),
        };
        groups.entry(base).or_default().push(i);
    }

    for (_base, indexs) in groups.into_iter() {
        if indexs.len() <= 1 {
            continue;
        }
        for i in indexs.into_iter() {
            if let SerialPortType::UsbPort(info) = &ports[i].port_type {
                ports[i].port_name = format!(
                    "{} (vid:{:04x} pid:{:04x})",
                    ports[i].port_name, info.vid, info.pid
                );
            }
        }
    }

    // Priority sort: Virtual ports (socat) first, then USB / ACM, then ttys
    fn priority(name: &str) -> i32 {
        let n = name.to_lowercase();
        if n.contains("vcom") || n.contains("tptyv") || n.contains("pts") && n.contains("v") {
            // Virtual ports created by socat get highest priority for testing
            -1
        } else if n.contains("ttyusb") || n.contains("usb") {
            0
        } else if n.contains("acm") {
            1
        } else if n.contains("ttys") || n.contains("serial") {
            2
        } else {
            10
        }
    }

    ports.sort_by(|a, b| {
        let pa = priority(&a.port_name);
        let pb = priority(&b.port_name);
        if pa != pb {
            pa.cmp(&pb)
        } else {
            a.port_name.cmp(&b.port_name)
        }
    });

    ports
}

/// Try to extract vid / pid / serial from a SerialPortType on Unix platforms.
pub fn try_extract_vid_pid_serial(pt: &serialport::SerialPortType) -> Option<VidPidSerial> {
    match pt {
        serialport::SerialPortType::UsbPort(info) => {
            let sn = info.serial_number.clone();
            let m = info.manufacturer.clone();
            let p = info.product.clone();
            Some((info.vid, info.pid, sn, m, p))
        }
        // Some serialport versions have different field names; try to fall back
        // To Debug parsing (best-effort).
        _ => {
            let dbg = format!("{:?}", pt).to_lowercase();
            let vid = parse_hex_after(&dbg, "vid");
            let pid = parse_hex_after(&dbg, "pid");
            let sn = parse_serial_after(&dbg, "serial")
                .or_else(|| parse_serial_after(&dbg, "serial_number"))
                .or_else(|| parse_serial_after(&dbg, "sn"));
            let manu = parse_string_after(&dbg, "manufacturer");
            let prod = parse_string_after(&dbg, "product");
            match (vid, pid) {
                (Some(v), Some(p)) => Some((v, p, sn, manu, prod)),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{SerialPortInfo, SerialPortType};

    fn make(name: &str) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.to_string(),
            port_type: SerialPortType::Unknown,
        }
    }

    #[test]
    fn unix_priority_and_annotation() {
        let input = vec![
            make("/dev/ttyS0"),
            make("/dev/ttyUSB0"),
            make("/dev/ttyACM0"),
            make("/dev/ttyS1"),
        ];
        let out = sort_and_dedup_ports(input);
        let names: Vec<_> = out.iter().map(|p| p.port_name.to_lowercase()).collect();
        let index_usb = names
            .iter()
            .position(|n| n.contains("ttyusb") || n.contains("usb"));
        let index_acm = names.iter().position(|n| n.contains("acm"));
        let index_ttys = names.iter().position(|n| {
            n.contains("ttys") || n.contains("ttys0") || n.contains("ttys1") || n.contains("ttys")
        });
        if let (Some(i_usb), Some(i_ttys)) = (index_usb, index_ttys) {
            assert!(i_usb < i_ttys);
        }
        if let (Some(i_acm), Some(i_ttys)) = (index_acm, index_ttys) {
            assert!(i_acm < i_ttys);
        }
    }
}
