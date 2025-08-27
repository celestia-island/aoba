use clap::ArgMatches;
use serde::Serialize;

#[derive(Serialize)]
struct PortInfo<'a> {
    port_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    vid: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_port_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    product: Option<String>,
}

/// Handle one-shot CLI actions. Return true if an action was handled and the
/// program should exit immediately.
pub fn run_one_shot_actions(matches: &ArgMatches) -> bool {
    if matches.get_flag("list-ports") {
        let ports = crate::protocol::tty::available_ports_sorted();

        let want_json = matches.get_flag("json");
        if want_json {
            let mut out: Vec<PortInfo> = Vec::new();
            for p in ports.iter() {
                // Try to extract vid/pid/serial using the existing helper if available
                let (vid, pid, serial, manufacturer, product) = match crate::protocol::tty::try_extract_vid_pid_serial(&p.port_type) {
                    Some((v, pa, s, m, pr)) => (Some(v), Some(pa), s, m, pr),
                    None => (None, None, None, None, None),
                };
                // Attempt to capture annotation if present in port_name (parenthetical)
                let ann = if p.port_name.contains('(') && p.port_name.contains(')') {
                    Some(p.port_name.clone())
                } else {
                    None
                };
                // Canonical: COMn if present, else basename for unix-like
                let canonical = compute_canonical(&p.port_name);
                let raw_type = Some(format!("{:?}", p.port_type));
                out.push(PortInfo {
                    port_name: &p.port_name,
                    vid,
                    pid,
                    serial,
                    annotation: ann,
                    canonical,
                    raw_port_type: raw_type,
                    manufacturer,
                    product,
                });
            }
            if let Ok(s) = serde_json::to_string_pretty(&out) {
                println!("{}", s);
            } else {
                    // Fallback to plain listing
                for p in ports.iter() {
                    println!("{}", p.port_name);
                }
            }
        } else {
            for p in ports.iter() {
                println!("{}", p.port_name);
            }
        }
        return true;
    }

    false
}

fn compute_canonical(name: &str) -> Option<String> {
    // Try to find COM<number> anywhere (case-insensitive)
    let up = name.to_uppercase();
    if let Some(pos) = up.find("COM") {
        let tail = &up[pos + 3..];
        let mut num = String::new();
        for c in tail.chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else {
                break;
            }
        }
        if !num.is_empty() {
            return Some(format!("COM{}", num));
        }
    }
    // Fallback: take basename after last '/'
    if let Some(b) = name.rsplit('/').next() {
        return Some(b.to_string());
    }
    None
}
