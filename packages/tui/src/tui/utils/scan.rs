use anyhow::{anyhow, Result};

use crate::tui::{
    status::port::{PortData, PortState},
    utils::bus::CoreToUi,
};
use aoba_protocol::tty::available_ports_sorted;

/// Perform a ports scan and update status. Returns Ok(true) if a scan ran, Ok(false) if skipped
/// because another scan was already in progress.
///
/// This function enumerates available serial ports and updates the TUI status with the results.
/// In CI debug mode (when --debug-ci-e2e-test is set), only virtual ports (vcom) are enumerated.
/// In normal mode, all available serial ports are enumerated using serialport library.
pub fn scan_ports(core_tx: &flume::Sender<CoreToUi>, scan_in_progress: &mut bool) -> Result<bool> {
    if *scan_in_progress {
        log::trace!("scan_ports: skipped (scan already in progress)");
        return Ok(false);
    }

    *scan_in_progress = true;
    log::info!("🔍 scan_ports: STARTING port enumeration and occupation check");

    // Enumerate ports using platform-specific function that respects CI debug mode
    let ports = available_ports_sorted();

    log::info!("scan_ports: enumerated {} ports from system", ports.len());

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
                // Port already exists, preserve its data but reset state for re-checking
                let mut preserved = existing_port.clone();
                preserved.port_type = port_type.clone();

                // CRITICAL: Only preserve state if port is occupied by THIS program
                // For other states (Free, OccupiedByOther), reset to Free and re-check
                // This prevents stale occupation status from being preserved
                if !preserved.state.is_occupied_by_this() {
                    log::trace!(
                        "Port {} state reset to Free for re-checking (was: {:?})",
                        port_name,
                        preserved.state
                    );
                    preserved.state = PortState::Free;
                }

                new_map.insert(port_name.clone(), preserved);
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
        log::debug!(
            "scan_ports: updating global status with {} ports (order: {:?})",
            new_order.len(),
            new_order
        );

        // Log state of each port before updating
        for port_name in &new_order {
            if let Some(port) = new_map.get(port_name) {
                log::debug!(
                    "  Port {}: state={:?}, type={}, has_config={}",
                    port_name,
                    port.state,
                    port.port_type,
                    match &port.config {
                        crate::tui::status::port::PortConfig::Modbus { stations, .. } =>
                            !stations.is_empty(),
                    }
                );
            }
        }

        status.ports.order = new_order;
        status.ports.map = new_map;

        Ok(())
    })?;

    // Check port occupation status using CLI subprocess
    // Important: Check ALL ports in the global status, not just enumerated ones
    // This ensures cached ports are also checked for occupation
    log::debug!("scan_ports: checking port occupation via CLI subprocess");
    check_all_ports_occupation_via_cli()?;

    core_tx
        .send(CoreToUi::Refreshed)
        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;

    *scan_in_progress = false;
    log::info!("✅ scan_ports: COMPLETED successfully");
    Ok(true)
}

/// Check occupation status for ALL ports in global status
/// This includes both enumerated ports and cached/manually added ports
fn check_all_ports_occupation_via_cli() -> Result<()> {
    use std::process::Command;

    // Get all port names from global status
    let all_ports = crate::tui::status::read_status(|status| Ok(status.ports.order.clone()))?;

    log::info!(
        "check_all_ports_occupation_via_cli: checking {} port(s) for occupation",
        all_ports.len()
    );

    if all_ports.is_empty() {
        log::debug!("No ports to check");
        return Ok(());
    }

    // Get the current executable path to spawn CLI
    let exe_path = std::env::current_exe()
        .map_err(|e| anyhow!("Failed to get current executable path: {e}"))?;

    let mut state_changes: Vec<(String, PortState)> = Vec::new();

    for port_name in &all_ports {
        // Skip ports already occupied by this TUI process
        let is_occupied_by_this = crate::tui::status::read_status(|status| {
            Ok(status
                .ports
                .map
                .get(port_name)
                .map(|p| p.state.is_occupied_by_this())
                .unwrap_or(false))
        })?;

        if is_occupied_by_this {
            log::trace!(
                "Skipping occupation check for {port_name} (occupied by this)"
            );
            continue;
        }

        // Spawn CLI subprocess to check port
        log::debug!(
            "Checking port {port_name} via CLI subprocess"
        );

        let output = Command::new(&exe_path)
            .arg("--check-port")
            .arg(port_name)
            .output();

        match output {
            Ok(result) => {
                let is_occupied = !result.status.success(); // exit 0 = free, 1 = occupied
                let exit_code = result.status.code().unwrap_or(-1);
                log::debug!(
                    "Port {port_name} CLI check completed: exit_code={exit_code}, is_occupied={is_occupied}"
                );

                // Read current state
                let current_state = crate::tui::status::read_status(|status| {
                    Ok(status
                        .ports
                        .map
                        .get(port_name)
                        .map(|p| p.state.clone())
                        .unwrap_or(PortState::Free))
                })?;

                // Determine new state
                let new_state = if is_occupied {
                    PortState::OccupiedByOther
                } else {
                    PortState::Free
                };

                // Record state change if different
                if current_state != new_state {
                    log::info!(
                        "Port {port_name} occupation state changed: {current_state:?} -> {new_state:?}"
                    );
                    state_changes.push((port_name.clone(), new_state));
                } else {
                    log::debug!(
                        "Port {port_name} occupation state unchanged: {current_state:?}"
                    );
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to spawn CLI subprocess for {port_name}: {e}"
                );
            }
        }
    }

    // Update all changed ports
    if !state_changes.is_empty() {
        log::info!(
            "check_all_ports_occupation_via_cli: updating {} port(s) with new occupation state",
            state_changes.len()
        );
        crate::tui::status::write_status(move |status| {
            for (port_name, new_state) in &state_changes {
                if let Some(port) = status.ports.map.get_mut(port_name) {
                    port.state = new_state.clone();
                    log::info!(
                        "Updated port {port_name} state to {new_state:?}"
                    );
                }
            }
            Ok(())
        })?;
    } else {
        log::debug!("check_all_ports_occupation_via_cli: no port state changes detected");
    }

    Ok(())
}
