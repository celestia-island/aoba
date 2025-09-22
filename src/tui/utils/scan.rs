use anyhow::{anyhow, Result};
use chrono::Local;

use crate::{
    protocol::{
        runtime::RuntimeEvent,
        status::{
            read_status,
            types::{
                self,
                port::{PortData, PortLogEntry, PortState},
            },
            with_port_read, with_port_write, write_status,
        },
        tty::available_ports_enriched,
    },
    tui::utils::bus::CoreToUi,
};

/// Perform a ports scan and update status. Returns Ok(true) if a scan ran, Ok(false) if skipped
/// because another scan was already in progress.
pub fn scan_ports(core_tx: &flume::Sender<CoreToUi>, scan_in_progress: &mut bool) -> Result<bool> {
    // Return early if scan already in progress
    if *scan_in_progress {
        return Ok(false);
    }

    *scan_in_progress = true;

    // Set busy indicator
    // We'll collect runtime handles for removed ports while holding the write lock,
    // but perform the potentially-blocking Stop+wait operations after releasing it.
    let mut to_stop: Vec<(String, crate::protocol::runtime::PortRuntimeHandle)> = Vec::new();
    let mut to_remove_names: Vec<String> = Vec::new();

    write_status(|status| {
        status.temporarily.busy.busy = true;
        Ok(())
    })?;

    let ports = available_ports_enriched();
    let scan_text = ports
        .iter()
        .map(|(info, extra)| format!("{} {:?}", info.port_name, extra))
        .collect::<Vec<_>>()
        .join("\n");

    write_status(|status| {
        // Update existing s.ports.map in-place to preserve PortData instances
        // (notably any PortState::OccupiedByThis runtime handles). Insert new
        // entries for newly discovered ports and remove entries for ports no
        // longer present.
        let mut new_order: Vec<String> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (info, extra) in ports.iter() {
            let name = info.port_name.clone();
            // Update existing PortData in-place if present
            if let Some(port) = status.ports.map.get(&name) {
                // update in-place using helper to avoid unwrap panics
                if with_port_write(port, |port| {
                    port.extra = extra.clone();
                    port.port_type = format!("{:?}", info.port_type);
                    port.port_name = name.clone();
                })
                .is_some()
                {
                    // updated
                } else {
                    log::warn!("scan_ports: failed to acquire write lock for port {name}");
                }
            } else {
                // Insert a new PortData for newly discovered port (wrap in Arc<RwLock<>>)
                let pd = PortData {
                    port_name: name.clone(),
                    port_type: format!("{:?}", info.port_type),
                    extra: extra.clone(),
                    state: PortState::Free,
                    ..Default::default()
                };
                status.ports.map.insert(
                    name.clone(),
                    std::sync::Arc::new(std::sync::RwLock::new(pd)),
                );
            }

            new_order.push(name.clone());
            seen.insert(name);
        }

        // Determine ports that disappeared since last scan. We'll collect their
        // names and any runtime handles that need stopping, then perform stop
        // outside of this write lock to avoid blocking other writers.
        let to_remove: Vec<String> = status
            .ports
            .map
            .keys()
            .filter(|k| !seen.contains(*k))
            .cloned()
            .collect();

        for key in to_remove.iter() {
            if let Some(port) = status.ports.map.get(key) {
                if let Some(opt_rt) = with_port_read(port, |port| match &port.state {
                    PortState::OccupiedByThis { runtime, .. } => Some(runtime.clone()),
                    _ => None,
                }) {
                    if let Some(rt) = opt_rt {
                        // Clone runtime handle for stopping later outside the write lock
                        to_stop.push((key.clone(), rt));
                    }
                } else {
                    log::warn!("scan_ports: failed to acquire read lock for port {key}");
                }
            }
            to_remove_names.push(key.clone());
        }

        // Update order for now (may be trimmed again after removals)
        status.ports.order = new_order;

        status.temporarily.scan.last_scan_time = Some(Local::now());
        status.temporarily.scan.last_scan_info = scan_text.clone();
        // Clear busy indicator after scan completes
        status.temporarily.busy.busy = false;
        Ok(())
    })?;

    // Now perform Stop + wait for each runtime outside the write lock to avoid
    // blocking other status writers. We still collected the runtime handles
    // while holding the write lock above.
    for (name, rt) in to_stop.into_iter() {
        let _ = rt
            .cmd_tx
            .send(crate::protocol::runtime::RuntimeCommand::Stop);
        let mut stopped = false;
        for _ in 0..10 {
            match rt
                .evt_rx
                .recv_timeout(std::time::Duration::from_millis(100))
            {
                Ok(evt) => {
                    if let crate::protocol::runtime::RuntimeEvent::Stopped = evt {
                        stopped = true;
                        break;
                    }
                }
                Err(_) => {
                    // timeout interval, continue waiting
                }
            }
        }
        if !stopped {
            log::warn!("scan_ports: stop did not emit Stopped event within timeout for {name}",);
        }
    }

    // After stopping runtimes, remove disappeared ports from the map and
    // update order to remove references to them.
    if !to_remove_names.is_empty() {
        write_status(|status| {
            for key in to_remove_names.iter() {
                status.ports.map.remove(key);
            }
            // Trim order to remove deleted names
            status.ports.order.retain(|n| !to_remove_names.contains(n));
            Ok(())
        })?;
    }

    *scan_in_progress = false;

    // After adding ports to status, spawn per-port runtime listeners.
    let ports_order = read_status(|status| Ok(status.ports.order.clone()))?;
    for port_name in ports_order.iter() {
        if let Some(pd_arc) = read_status(|status| Ok(status.ports.map.get(port_name).cloned()))? {
            if let Some(opt_rt) = with_port_read(&pd_arc, |pd| match &pd.state {
                types::port::PortState::OccupiedByThis { runtime, .. } => Some(runtime.clone()),
                _ => None,
            }) {
                if let Some(runtime) = opt_rt {
                    let evt_rx = runtime.evt_rx.clone();
                    let pn = port_name.clone();
                    std::thread::spawn(move || {
                        if let Err(err) = spawn_runtime_listener(evt_rx, pn.clone()) {
                            log::warn!(
                                "spawn_runtime_listener for {pn} exited with error: {err:?}",
                            );
                        }
                    });
                }
            } else {
                log::warn!("scan_ports: failed to acquire read lock for port {port_name}");
            }
        }
    }

    core_tx
        .send(CoreToUi::Refreshed)
        .map_err(|e| anyhow!("failed to send Refreshed: {}", e))?;
    Ok(true)
}

/// Spawn a detached thread that listens on a runtime's evt_rx and writes port logs into status.
fn spawn_runtime_listener(evt_rx: flume::Receiver<RuntimeEvent>, port_name: String) -> Result<()> {
    const MAX_LOGS: usize = 2000;
    while let Ok(evt) = evt_rx.recv() {
        match evt {
            RuntimeEvent::FrameReceived(b) | RuntimeEvent::FrameSent(b) => {
                let now = Local::now();
                let raw = b
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let parsed = Some(format!("{} bytes", b.len()));
                let entry = PortLogEntry {
                    when: now,
                    raw,
                    parsed,
                };
                write_status(|status| {
                    if let Some(port) = status.ports.map.get(&port_name) {
                        if with_port_write(port, |port| {
                            port.logs.push(entry.clone());
                            if port.logs.len() > MAX_LOGS {
                                let drop = port.logs.len() - MAX_LOGS;
                                port.logs.drain(0..drop);
                            }
                        })
                        .is_some()
                        {
                            // written
                        } else {
                            log::warn!("spawn_runtime_listener: failed to acquire write lock for {port_name}");
                        }
                    }
                    Ok(())
                })?;
            }
            _ => {}
        }
    }
    Ok(())
}
