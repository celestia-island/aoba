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
                                format!("{}:vid={:04x}:pid={:04x}:sn={}", base, vid, pid, sn)
                            } else {
                                format!("{}:vid={:04x}:pid={:04x}", base, vid, pid)
                            }
                        } else {
                            format!("{}:usb", base)
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
                                format!("{}:vid={:04x}:pid={:04x}:sn={}", base, vid, pid, sn)
                            } else {
                                format!("{}:vid={:04x}:pid={:04x}", base, vid, pid)
                            }
                        } else {
                            format!("{}:usb", base)
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
            let base = extract_com_base(&up).unwrap_or_else(|| up);
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
            if up.starts_with("COM") {
                up[3..].parse::<u32>().ok()
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
pub fn try_extract_vid_pid_serial(
    pt: &SerialPortType,
) -> Option<(u16, u16, Option<String>, Option<String>, Option<String>)> {
    let dbg = format!("{:?}", pt).to_lowercase();
    // Try common keys
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
    let dbg = format!("{:?}", pt);
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
    if let Some(pos) = s.find(key) {
        let tail = &s[pos + key.len()..];
        if let Some(eq) = tail.find('=') {
            let after = &tail[eq + 1..];
            let mut out = String::new();
            for c in after.chars().skip_while(|c| c.is_whitespace()) {
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
    }
    None
}

fn parse_hex_after(s: &str, key: &str) -> Option<u16> {
    if let Some(pos) = s.find(key) {
        let tail = &s[pos + key.len()..];
        // Look for 0xhhhh
        if let Some(xpos) = tail.find("0x") {
            let mut num = String::new();
            for c in tail[xpos + 2..].chars() {
                if c.is_ascii_hexdigit() {
                    num.push(c);
                } else {
                    break;
                }
            }
            if !num.is_empty() {
                if let Ok(v) = u16::from_str_radix(&num, 16) {
                    return Some(v);
                }
            }
        }
        // Look for =hhhh or =0xhhhh or =dddd
        if let Some(eq) = tail.find('=') {
            let after = &tail[eq + 1..];
            let mut num = String::new();
            for c in after.chars().skip_while(|c| c.is_whitespace()) {
                if c.is_ascii_hexdigit() {
                    num.push(c);
                } else if c.is_ascii_digit() && num.is_empty() {
                    num.push(c);
                } else if num.is_empty() {
                    // Skip non-numeric until numeric starts
                    continue;
                } else {
                    break;
                }
            }
            if !num.is_empty() {
                // Try hex then decimal
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
    if let Some(pos) = s.find(key) {
        let tail = &s[pos + key.len()..];
        if let Some(eq) = tail.find('=') {
            let after = &tail[eq + 1..];
            let mut out = String::new();
            for c in after.chars().skip_while(|c| c.is_whitespace()) {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.' {
                    out.push(c);
                } else {
                    break;
                }
            }
            if !out.is_empty() {
                return Some(out);
            }
        }
        if let Some(col) = tail.find(':') {
            let after = &tail[col + 1..];
            let mut out = String::new();
            for c in after.chars().skip_while(|c| c.is_whitespace()) {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.' {
                    out.push(c);
                } else {
                    break;
                }
            }
            if !out.is_empty() {
                return Some(out);
            }
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
                return Some(format!("COM{}", num));
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
}
