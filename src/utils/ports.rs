use anyhow::Result;
use std::{path::Path, process::Command};

/// Return a sorted list of available ports as (port_name, port_type_string).
pub fn enumerate_ports() -> Vec<(String, String)> {
    // Use unified platform-specific enumeration which includes virtual port
    // detection when CI/debug hints are enabled.
    let mut ports = crate::protocol::tty::available_ports_sorted();
    ports.sort_by_key(|p| p.port_name.clone());
    ports
        .into_iter()
        .map(|p| (p.port_name.clone(), format!("{:?}", p.port_type)))
        .collect()
}

/// Check occupation status for a list of ports by invoking the current executable
/// with `--check-port <port>` for each port. Returns a vector of (port_name, is_occupied).
pub fn check_ports_occupied(exe_path: &Path, ports: &[String]) -> Result<Vec<(String, bool)>> {
    let mut results: Vec<(String, bool)> = Vec::new();

    if ports.is_empty() {
        return Ok(results);
    }

    for port_name in ports {
        let output = Command::new(exe_path)
            .arg("--check-port")
            .arg(port_name)
            .output();
        match output {
            Ok(result) => {
                let is_occupied = !result.status.success();
                results.push((port_name.clone(), is_occupied));
            }
            Err(e) => {
                log::warn!("Failed to spawn CLI subprocess for {}: {}", port_name, e);
                // on error, conservatively assume free
                results.push((port_name.clone(), false));
            }
        }
    }

    Ok(results)
}

/// A lightweight snapshot of a previously-known port used for merge decisions.
#[derive(Debug, Clone)]
pub struct PreviousPort {
    pub name: String,
    pub occupied_by_this: bool,
    pub has_config: bool,
    pub log_count: usize,
}

/// Merge enumerated ports with previous ports according to preservation policy.
///
/// - `enumerated` is a slice of (name, port_type)
/// - `previous` is a slice of `PreviousPort` snapshots
///
/// Returns a vector of (name, Option<port_type>) in the desired order.
/// For enumerated ports the port_type is Some(..). For preserved-but-not-enumerated
/// ports the port_type will be None.
pub fn merge_enumeration(
    enumerated: &[(String, String)],
    previous: &[PreviousPort],
) -> Vec<(String, Option<String>)> {
    use std::collections::HashSet;

    let mut order: Vec<(String, Option<String>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // First, add enumerated ports (preserve order)
    for (name, ptype) in enumerated {
        seen.insert(name.clone());
        order.push((name.clone(), Some(ptype.clone())));
    }

    // Then, preserve previous ports that were not enumerated but meet preservation criteria
    for prev in previous {
        if seen.contains(&prev.name) {
            continue;
        }

        let should_preserve = prev.occupied_by_this || prev.has_config || prev.log_count > 0;
        if should_preserve {
            order.push((prev.name.clone(), None));
            seen.insert(prev.name.clone());
        }
    }

    order
}
