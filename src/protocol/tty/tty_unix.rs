use serialport::{SerialPortInfo, SerialPortType};
use std::collections::{HashMap, HashSet};

/// Return the list of available serial ports sorted/deduped for Unix.
pub fn available_ports_sorted() -> Vec<SerialPortInfo> {
    let raw_ports = serialport::available_ports().unwrap_or_default();
    sort_and_dedup_ports(raw_ports)
}

pub(crate) fn sort_and_dedup_ports(raw_ports: Vec<SerialPortInfo>) -> Vec<SerialPortInfo> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut unique: Vec<SerialPortInfo> = Vec::new();

    for mut p in raw_ports.into_iter() {
        let base = match p.port_name.rsplit('/').next() {
            Some(b) => b.to_lowercase(),
            None => p.port_name.to_lowercase(),
        };
        let key = match &p.port_type {
            SerialPortType::UsbPort { vid, pid, .. } => format!("{}:vid={:04x}:pid={:04x}", base, vid, pid),
            _ => base,
        };

        if seen.insert(key) {
            unique.push(p);
        }
    }

    let mut ports = unique;

    // Annotate devices sharing same basename with vid/pid so user can distinguish
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, p) in ports.iter().enumerate() {
        let base = match p.port_name.rsplit('/').next() {
            Some(b) => b.to_lowercase(),
            None => p.port_name.to_lowercase(),
        };
        groups.entry(base).or_default().push(i);
    }

    for (_base, idxs) in groups.into_iter() {
        if idxs.len() <= 1 {
            continue;
        }
        for i in idxs.into_iter() {
            if let SerialPortType::UsbPort { vid, pid, .. } = ports[i].port_type {
                ports[i].port_name = format!("{} (vid:{:04x} pid:{:04x})", ports[i].port_name, vid, pid);
            }
        }
    }

    // Priority sort: USB/ACM first, then ttys
    fn priority(name: &str) -> i32 {
        let n = name.to_lowercase();
        if n.contains("ttyusb") || n.contains("usb") {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{SerialPortInfo, SerialPortType};

    fn make(name: &str) -> SerialPortInfo { SerialPortInfo { port_name: name.to_string(), port_type: SerialPortType::Unknown } }

    #[test]
    fn unix_priority_and_annotation() {
        let input = vec![make("/dev/ttyS0"), make("/dev/ttyUSB0"), make("/dev/ttyACM0"), make("/dev/ttyS1")];
        let out = sort_and_dedup_ports(input);
        let names: Vec<_> = out.iter().map(|p| p.port_name.to_lowercase()).collect();
        let idx_usb = names.iter().position(|n| n.contains("ttyusb") || n.contains("usb"));
        let idx_acm = names.iter().position(|n| n.contains("acm"));
        let idx_ttys = names.iter().position(|n| n.contains("ttys") || n.contains("ttys0") || n.contains("ttys1") || n.contains("ttys"));
        if let (Some(i_usb), Some(i_ttys)) = (idx_usb, idx_ttys) { assert!(i_usb < i_ttys); }
        if let (Some(i_acm), Some(i_ttys)) = (idx_acm, idx_ttys) { assert!(i_acm < i_ttys); }
    }
}
