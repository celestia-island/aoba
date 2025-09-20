use anyhow::{anyhow, Result};
use chrono::Local;

use crate::{
    protocol::{
        runtime::RuntimeEvent,
        status::{
            read_status,
            types::port::{PortData, PortLogEntry, PortState},
            write_status,
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
    write_status(|s| {
        s.temporarily.busy.busy = true;
        Ok(())
    })?;

    let ports = available_ports_enriched();
    let scan_text = ports
        .iter()
        .map(|(info, extra)| format!("{} {:?}", info.port_name, extra))
        .collect::<Vec<_>>()
        .join("\n");

    write_status(|s| {
        s.ports.order.clear();
        s.ports.map.clear();
        for (info, extra) in ports.iter() {
            let pd = PortData {
                port_name: info.port_name.clone(),
                port_type: format!("{:?}", info.port_type),
                info: Some(info.clone()),
                extra: extra.clone(),
                state: PortState::Free,
                handle: None,
                runtime: None,
                ..Default::default()
            };

            s.ports.order.push(info.port_name.clone());
            s.ports.map.insert(info.port_name.clone(), pd);
        }

        s.temporarily.scan.last_scan_time = Some(Local::now());
        s.temporarily.scan.last_scan_info = scan_text.clone();
        // Clear busy indicator after scan completes
        s.temporarily.busy.busy = false;
        Ok(())
    })?;

    *scan_in_progress = false;

    // After adding ports to status, spawn per-port runtime listeners.
    let ports_order = read_status(|s| Ok(s.ports.order.clone()))?;
    for port_name in ports_order.iter() {
        if let Some(pd) = read_status(|s| Ok(s.ports.map.get(port_name).cloned()))? {
            if let Some(runtime) = pd.runtime {
                let evt_rx = runtime.evt_rx.clone();
                let pn = port_name.clone();
                std::thread::spawn(move || {
                    if let Err(err) = spawn_runtime_listener(evt_rx, pn.clone()) {
                        log::warn!(
                            "spawn_runtime_listener for {} exited with error: {:?}",
                            pn,
                            err
                        );
                    }
                });
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
                write_status(|s| {
                    if let Some(pdata) = s.ports.map.get_mut(&port_name) {
                        pdata.logs.push(entry.clone());
                        if pdata.logs.len() > MAX_LOGS {
                            let drop = pdata.logs.len() - MAX_LOGS;
                            pdata.logs.drain(0..drop);
                        }
                        if pdata.log_auto_scroll {
                            pdata.log_selected = pdata.logs.len().saturating_sub(1);
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
