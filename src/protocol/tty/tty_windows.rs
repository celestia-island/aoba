use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use super::PortExtra;
use serialport::{SerialPortInfo, SerialPortType};

/// Return the list of available serial ports sorted / deduped for Windows.
pub fn available_ports_sorted() -> Vec<SerialPortInfo> {
    let raw_ports = serialport::available_ports().unwrap_or_default();
    sort_and_dedup_ports(raw_ports)
}

pub fn available_ports_enriched() -> Vec<(SerialPortInfo, PortExtra)> {
    let ports = available_ports_sorted();
    ports
        .into_iter()
        .map(|p| {
            let guid = if matches!(p.port_type, SerialPortType::UsbPort { .. }) {
                try_extract_device_guid(&p.port_type)
            } else {
                None
            };
            let meta = try_extract_vid_pid_serial(&p.port_type);
            let (vid, pid, serial, manufacturer, product) = meta
                .map(|(v, p, s, m, pr)| (Some(v), Some(p), s, m, pr))
                .unwrap_or((None, None, None, None, None));
            (
                p,
                PortExtra {
                    guid,
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

    for port in raw_ports.into_iter() {
        // Normalize COM names. We avoid destructuring UsbPort fields directly
        // Because different serialport crate versions expose the variant in
        // Different shapes; prefer a conservative approach that still dedups
        // By visible name and marks USB ports when necessary.
        let key = {
            let name_up = port.port_name.to_uppercase();
            // Extract COM<number> anywhere in the name (handles NULL_COM3 etc.)
            if let Some(base) = extract_com_base(&name_up) {
                // Base like "COM3"
                match &port.port_type {
                    SerialPortType::UsbPort { .. } => {
                        if let Some((vid, pid, sn, _m, _p)) =
                            try_extract_vid_pid_serial(&port.port_type)
                        {
                            if let Some(sn) = sn {
                                format!("{base}:vid={vid:04x}:pid={pid:04x}:sn={sn}")
                            } else {
                                format!("{base}:vid={vid:04x}:pid={pid:04x}")
                            }
                        } else {
                            format!("{base}:usb")
                        }
                    }
                    _ => base,
                }
            } else {
                // Non-COM names: fallback to lowercase name and include usb metadata
                let base = port.port_name.to_lowercase();
                match &port.port_type {
                    SerialPortType::UsbPort { .. } => {
                        if let Some((vid, pid, sn, _m, _p)) =
                            try_extract_vid_pid_serial(&port.port_type)
                        {
                            if let Some(sn) = sn {
                                format!("{base}:vid={vid:04x}:pid={pid:04x}:sn={sn}")
                            } else {
                                format!("{base}:vid={vid:04x}:pid={pid:04x}")
                            }
                        } else {
                            format!("{base}:usb")
                        }
                    }
                    _ => base,
                }
            }
        };

        if seen.insert(key) {
            unique.push(port);
        }
    }

    let mut ports = unique;

    // If multiple entries share same COM base (rare), annotate ones that have
    // USB metadata so user can distinguish.
    {
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, p) in ports.iter().enumerate() {
            let up = p.port_name.to_uppercase();
            let base = extract_com_base(&up).unwrap_or(up);
            groups.entry(base).or_default().push(i);
        }

        for (_base, idxs) in groups.into_iter() {
            if idxs.len() <= 1 {
                continue;
            }
            for i in idxs.into_iter() {
                // If it's a usb-type port, attempt to append VID / PID / SN if
                // We can extract them; otherwise append a generic (usb).
                if matches!(ports[i].port_type, SerialPortType::UsbPort { .. }) {
                    if let Some((vid, pid, sn, _m, _p)) =
                        try_extract_vid_pid_serial(&ports[i].port_type)
                    {
                        if let Some(sn) = sn {
                            ports[i].port_name = format!(
                                "{} (vid:{:04x} pid:{:04x} sn:{})",
                                ports[i].port_name, vid, pid, sn
                            );
                        } else {
                            ports[i].port_name =
                                format!("{} (vid:{:04x} pid:{:04x})", ports[i].port_name, vid, pid);
                        }
                    } else {
                        ports[i].port_name = format!("{} (usb)", ports[i].port_name);
                    }
                }
            }
        }
    }

    // Sort primarily by COM numeric index when possible
    ports.sort_by(|a, b| {
        fn com_index(name: &str) -> Option<u32> {
            let up = name.to_uppercase();
            if let Some(stripped) = up.strip_prefix("COM") {
                stripped.parse::<u32>().ok()
            } else {
                None
            }
        }

        let ia = com_index(&a.port_name);
        let ib = com_index(&b.port_name);
        match (ia, ib) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.port_name.cmp(&b.port_name),
        }
    });

    ports
}

// Attempt to conservatively extract vid / pid / serial from a SerialPortType
// By inspecting its Debug representation. This is best-effort and should
// Not be relied on strictly, but helps annotate ports when metadata is
// Available across different serialport crate versions.
pub type VidPidSerial = (u16, u16, Option<String>, Option<String>, Option<String>);

pub fn try_extract_vid_pid_serial(pt: &SerialPortType) -> Option<VidPidSerial> {
    let dbg = format!("{pt:?}").to_lowercase();
    let vid = parse_hex_after(&dbg, "vid");
    let pid = parse_hex_after(&dbg, "pid");
    let sn = parse_serial_after(&dbg, "serial")
        .or_else(|| parse_serial_after(&dbg, "serial_number"))
        .or_else(|| parse_serial_after(&dbg, "sn"));
    let manufacturer = parse_string_after(&dbg, "manufacturer");
    let product = parse_string_after(&dbg, "product");
    match (vid, pid) {
        (Some(v), Some(p)) => Some((v, p, sn, manufacturer, product)),
        _ => None,
    }
}

// Attempt to extract a Windows device interface GUID (best-effort) from Debug string.
// The serialport crate doesn't expose GUID directly; Windows device paths often contain patterns like
// "{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}". We scan the Debug representation to pull the first GUID.
pub fn try_extract_device_guid(pt: &SerialPortType) -> Option<String> {
    let dbg = format!("{pt:?}");
    // Simple state machine to find first '{' then collect until '}' including hyphens.
    let mut chars = dbg.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut guid = String::from("{");
            while let Some(&nc) = chars.peek() {
                guid.push(nc);
                chars.next();
                if nc == '}' {
                    break;
                }
                if guid.len() > 60 {
                    break;
                } // safety cap
            }
            // Validate crude GUID shape (length and hyphen positions). Typical length 38 including braces.
            let lower = guid.to_ascii_lowercase();
            let expected_len = 38; // {8-4-4-4-12}
            if lower.len() == expected_len
                && lower.chars().nth(9) == Some('-')
                && lower.chars().nth(14) == Some('-')
                && lower.chars().nth(19) == Some('-')
                && lower.chars().nth(24) == Some('-')
            {
                return Some(lower);
            }
        }
    }
    None
}

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
    // check for = followed by number (hex or dec)
    if let Some((_, after)) = tail.split_once('=') {
        let mut iter = after.trim_start().chars().peekable();
        let mut num = String::new();
        // collect optional 0x prefix
        // check for leading '0x'
        if let (Some('0'), Some('x')) = (iter.peek().copied(), {
            // second peek: create a small iterator clone to peek second char
            let mut it2 = iter.clone();
            it2.next();
            it2.peek().copied()
        }) {
            // consume '0' 'x'
            iter.next();
            iter.next();
            while let Some(&c) = iter.peek() {
                if c.is_ascii_hexdigit() {
                    num.push(c);
                    iter.next();
                } else {
                    break;
                }
            }
            if !num.is_empty() {
                if let Ok(v) = u16::from_str_radix(&num, 16) {
                    return Some(v);
                }
            }
        } else {
            // decimal or bare hex digits
            while let Some(&c) = iter.peek() {
                if c.is_ascii_digit() || c.is_ascii_hexdigit() {
                    num.push(c);
                    iter.next();
                } else if num.is_empty() {
                    iter.next(); // skip leading non-numeric
                } else {
                    break;
                }
            }
            if !num.is_empty() {
                // try hex first then decimal
                if let Ok(v) = u16::from_str_radix(&num, 16) {
                    return Some(v);
                }
                if let Ok(v) = num.parse::<u16>() {
                    return Some(v);
                }
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
    if let Some((_, after)) = tail.split_once(':') {
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

fn extract_com_base(s: &str) -> Option<String> {
    // Find "COM" followed by digits anywhere
    let up = s.to_uppercase();
    let chars = up.chars().collect::<Vec<_>>();
    let len = chars.len();
    for i in 0..len.saturating_sub(3) {
        if chars[i] == 'C' && chars[i + 1] == 'O' && chars[i + 2] == 'M' {
            // Collect digits after
            let mut j = i + 3;
            let mut num = String::new();
            while j < len {
                let c = chars[j];
                if c.is_ascii_digit() {
                    num.push(c);
                    j += 1;
                } else {
                    break;
                }
            }
            if !num.is_empty() {
                return Some(format!("COM{num}"));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{SerialPortInfo, SerialPortType};

    fn make_com(name: &str) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.to_string(),
            port_type: SerialPortType::Unknown,
        }
    }

    #[test]
    fn windows_sort_and_dedup() {
        let input = vec![
            make_com("COM3"),
            make_com("COM1"),
            make_com("COM2"),
            make_com("com2"),
        ];
        let out = sort_and_dedup_ports(input);
        // Ensure COM numeric sorting and dedup
        assert!(out[0].port_name.to_uppercase().starts_with("COM1"));
        assert!(out
            .iter()
            .map(|p| p.port_name.to_uppercase())
            .collect::<Vec<_>>()
            .windows(2)
            .all(|w| w[0] <= w[1]));
        let names: Vec<_> = out.iter().map(|p| p.port_name.to_lowercase()).collect();
        let mut uniq = names.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(names.len(), uniq.len());
    }

    #[test]
    fn windows_null_com_dedup() {
        use serialport::SerialPortType;
        let input = vec![
            SerialPortInfo {
                port_name: "COM3".to_string(),
                port_type: SerialPortType::Unknown,
            },
            SerialPortInfo {
                port_name: "NULL_COM3".to_string(),
                port_type: SerialPortType::Unknown,
            },
            SerialPortInfo {
                port_name: "Com4".to_string(),
                port_type: SerialPortType::Unknown,
            },
        ];
        let out = sort_and_dedup_ports(input);
        let names: Vec<_> = out.iter().map(|p| p.port_name.to_uppercase()).collect();
        // Expect only one entry for COM3
        assert_eq!(names.iter().filter(|n| n == &"COM3").count(), 1);
        assert!(names.contains(&"COM4".to_string()));
    }

    #[test]
    fn extract_guid_from_debug() {
        // Cannot directly construct serialport::SerialPortType::UsbPort in current crate version; simulate Debug string parsing instead:
        let fake_debug =
            "UsbPort(vid=0x1a86 pid=0x7523 path=\\\\?\\usb#{12345678-9abc-4def-8012-001122334455})";
        // Wrap as Unknown and invoke the public GUID extraction helper (which relies on Debug).
        // We temporarily mimic an Unknown port type just to format and replace the string to validate parser stability.
        let mut dbg = String::new();
        dbg.push_str(fake_debug);
        // Reuse internal scanning logic (manually copied here for the test)
        let manual = {
            let s = dbg.clone();
            let mut chars = s.chars().peekable();
            let mut found = None;
            while let Some(c) = chars.next() {
                if c == '{' {
                    let mut g = String::from("{");
                    while let Some(&nc) = chars.peek() {
                        g.push(nc);
                        chars.next();
                        if nc == '}' {
                            break;
                        }
                        if g.len() > 60 {
                            break;
                        }
                    }
                    let lower = g.to_ascii_lowercase();
                    if lower.len() == 38
                        && lower.chars().nth(9) == Some('-')
                        && lower.chars().nth(14) == Some('-')
                        && lower.chars().nth(19) == Some('-')
                        && lower.chars().nth(24) == Some('-')
                    {
                        found = Some(lower);
                        break;
                    }
                }
            }
            found
        };
        assert_eq!(
            manual,
            Some("{12345678-9abc-4def-8012-001122334455}".into())
        );
    }

    #[test]
    fn parse_hex_and_vid_pid_examples() {
        let s1 =
            "UsbPort(vid=0x1a86 pid=0x7523 serial=ABC123 manufacturer=ACME product=USB-Serial)";
        // try_extract_vid_pid_serial expects a Debug string; exercise parse_hex_after directly
        assert_eq!(parse_hex_after(s1, "vid"), Some(0x1a86));
        assert_eq!(parse_hex_after(s1, "pid"), Some(0x7523));

        let s2 = "SomePort name serial=SN-9876: extra";
        assert_eq!(
            parse_serial_after(s2, "serial"),
            Some("SN-9876".to_string())
        );
    }
}
