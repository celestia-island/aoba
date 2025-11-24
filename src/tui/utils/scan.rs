use anyhow::{anyhow, Result};

use crate::{
    core::bus::CoreToUi,
    tui::status::port::{PortData, PortState},
    utils::ports::enumerate_ports,
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
    log::info!("🔍 scan_ports: STARTING port enumeration and occupation check");

    // Enumerate ports using shared util (respects CI debug mode)
    let ports = enumerate_ports();

    log::info!("scan_ports: enumerated {} ports from system", ports.len());

    // Clone port names before moving into closure
    let port_names_and_types: Vec<(String, String)> = ports.into_iter().collect();

    // Update status with discovered ports
    crate::tui::status::write_status(|status| {
        // Build new port order and map
        let mut new_order = Vec::new();
        let mut new_map = std::collections::HashMap::new();

        // First, process enumerated ports
        for (port_name, _port_type) in &port_names_and_types {
            new_order.push(port_name.clone());

            // Check if port already exists in status
            if let Some(existing_port) = status.ports.map.get(port_name) {
                // Port already exists, preserve its data but reset state for re-checking
                let mut preserved = existing_port.clone();
                preserved.port_type =
                    crate::protocol::status::types::port::PortType::detect(port_name);

                // CRITICAL: Only preserve state if port is occupied by THIS program
                // For other states (Free, OccupiedByOther), reset to Free and re-check
                // This prevents stale occupation status from being preserved
                if !preserved.state.is_occupied_by_this() {
                    preserved.state = PortState::Free;
                }

                new_map.insert(port_name.clone(), preserved);
            } else {
                // New port discovered, create PortData with default values
                let port_data = PortData {
                    port_name: port_name.clone(),
                    port_type: crate::protocol::status::types::port::PortType::detect(port_name),
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
                // 1. It's a virtual/IPC port (not enumerated by serialport)
                // 2. It's occupied (being used)
                // 3. It has modbus configuration
                // 4. It has logs
                use crate::tui::status::port::PortConfig;
                let has_config = match &old_port_data.config {
                    PortConfig::Modbus { stations, .. } => !stations.is_empty(),
                };

                // Check if this is a virtual port using PortType enum
                let is_virtual_port =
                    old_port_data.port_type.is_virtual() || old_port_name.contains("vcom");

                let should_preserve = is_virtual_port  // Always preserve IPC/virtual ports
                    || !matches!(old_port_data.state, PortState::Free)
                    || has_config
                    || !old_port_data.logs.is_empty();

                if should_preserve {
                    new_order.push(old_port_name.clone());
                    new_map.insert(old_port_name.clone(), old_port_data.clone());
                }
            }
        }

        // Update status with new port list

        // Log state of each port before updating

        status.ports.order = new_order;
        status.ports.map = new_map;

        Ok(())
    })?;

    // Check port occupation status using CLI subprocess via shared util
    // Important: Check ALL ports in the global status, not just enumerated ones
    // This ensures cached ports are also checked for occupation

    // Prepare previous-port snapshots for merge policy
    let previous_ports_snapshot: Vec<crate::utils::ports::PreviousPort> =
        crate::tui::status::read_status(|status| {
            Ok(status
                .ports
                .order
                .iter()
                .map(|name| {
                    let p = status.ports.map.get(name).unwrap();
                    crate::utils::ports::PreviousPort {
                        name: name.clone(),
                        occupied_by_this: p.state.is_occupied_by_this(),
                        has_config: match &p.config {
                            crate::tui::status::port::PortConfig::Modbus { stations, .. } => {
                                !stations.is_empty()
                            }
                        },
                        log_count: p.logs.len(),
                    }
                })
                .collect::<Vec<_>>())
        })?;

    // Merge enumerated with previous using shared policy
    let merged =
        crate::utils::ports::merge_enumeration(&port_names_and_types, &previous_ports_snapshot);

    // Now construct new order and map using merged result. If a port had a known type from enumeration use it;
    // otherwise preserve existing PortData where present.
    let mut new_order = Vec::new();
    let mut new_map = std::collections::HashMap::new();

    for (name, opt_type) in merged {
        new_order.push(name.clone());
        if let Some(_ptype) = opt_type {
            // If enumerated, either preserve existing data or create new PortData
            if let Some(existing) =
                crate::tui::status::read_status(|status| Ok(status.ports.map.get(&name).cloned()))?
            {
                let mut preserved = existing.clone();
                preserved.port_type = crate::protocol::status::types::port::PortType::detect(&name);
                if !preserved.state.is_occupied_by_this() {
                    preserved.state = PortState::Free;
                }
                new_map.insert(name.clone(), preserved);
            } else {
                let port_data = PortData {
                    port_name: name.clone(),
                    port_type: crate::protocol::status::types::port::PortType::detect(&name),
                    state: PortState::Free,
                    ..Default::default()
                };
                new_map.insert(name.clone(), port_data);
            }
        } else {
            // Preserved but not enumerated - copy existing if present
            if let Some(existing) =
                crate::tui::status::read_status(|status| Ok(status.ports.map.get(&name).cloned()))?
            {
                new_map.insert(name.clone(), existing.clone());
            }
        }
    }

    core_tx
        .send(CoreToUi::Refreshed)
        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;

    *scan_in_progress = false;
    log::info!("✅ scan_ports: COMPLETED successfully");
    Ok(true)
}
