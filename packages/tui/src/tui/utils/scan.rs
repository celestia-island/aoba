use anyhow::{anyhow, Result};

use crate::tui::{
    status::port::{PortData, PortState},
    utils::bus::CoreToUi,
};

/// Perform a ports scan and update status. Returns Ok(true) if a scan ran, Ok(false) if skipped
/// because another scan was already in progress.
///
/// This function enumerates available serial ports and updates the TUI status with the results.
/// In CI debug mode (when --debug-ci-e2e-test is set), only virtual ports (vcom) are enumerated.
/// In normal mode, all available serial ports are enumerated using serialport library.
pub fn scan_ports(core_tx: &flume::Sender<CoreToUi>, scan_in_progress: &mut bool) -> Result<bool> {
    if *scan_in_progress {
        return Ok(false);
    }

    *scan_in_progress = true;

    // Enumerate ports using platform-specific function that respects CI debug mode
    let ports = crate::protocol::tty::available_ports_sorted();

    log::debug!("scan_ports: found {} ports", ports.len());

    // Clone port names before moving into closure
    let port_names_and_types: Vec<(String, String)> = ports
        .into_iter()
        .map(|p| (p.port_name.clone(), format!("{:?}", p.port_type)))
        .collect();

    // Update status with discovered ports
    crate::tui::status::write_status(|status| {
        // Build new port order and map
        let mut new_order = Vec::new();
        let mut new_map = std::collections::HashMap::new();

        // First, process enumerated ports
        for (port_name, port_type) in &port_names_and_types {
            new_order.push(port_name.clone());

            // Check if port already exists in status
            if let Some(existing_port) = status.ports.map.get(port_name) {
                // Port already exists, preserve its entire state (config, logs, etc.)
                new_map.insert(port_name.clone(), existing_port.clone());
            } else {
                // New port discovered, create PortData with default values
                let port_data = PortData {
                    port_name: port_name.clone(),
                    port_type: port_type.clone(),
                    state: PortState::Free,
                    ..Default::default()
                };
                new_map.insert(port_name.clone(), port_data);
            }
        }

        // Second, preserve ports that were in the old map but not enumerated
        // These might be manually added ports or ports that temporarily disappeared
        // Only preserve them if they are NOT in Free state or have configuration
        for (old_port_name, old_port_data) in &status.ports.map {
            if !new_map.contains_key(old_port_name) {
                // Port was not in the new enumeration
                // Preserve it if:
                // 1. It's occupied (being used)
                // 2. It has modbus configuration
                // 3. It has logs
                use crate::tui::status::port::PortConfig;
                let has_config = match &old_port_data.config {
                    PortConfig::Modbus { stations, .. } => !stations.is_empty(),
                };

                let should_preserve = !matches!(old_port_data.state, PortState::Free)
                    || has_config
                    || !old_port_data.logs.is_empty();

                if should_preserve {
                    log::debug!(
                        "scan_ports: preserving non-enumerated port {} (state={:?}, has_config={})",
                        old_port_name,
                        old_port_data.state,
                        has_config
                    );
                    new_order.push(old_port_name.clone());
                    new_map.insert(old_port_name.clone(), old_port_data.clone());
                }
            }
        }

        // Update status with new port list
        status.ports.order = new_order;
        status.ports.map = new_map;

        Ok(())
    })?;

    core_tx
        .send(CoreToUi::Refreshed)
        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;

    *scan_in_progress = false;
    Ok(true)
}
