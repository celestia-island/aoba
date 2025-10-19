/// Debug dump utilities for CI/E2E testing
///
/// This module provides functionality to periodically dump the global status tree
/// to a file for external monitoring during E2E tests.
use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use anyhow::Result;
use serde::Serialize;

use crate::protocol::status::read_status;

/// Flag to control whether debug dumping is enabled
static DEBUG_DUMP_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable debug dump mode (should be called when --debug-ci-e2e-test is set)
pub fn enable_debug_dump() {
    DEBUG_DUMP_ENABLED.store(true, Ordering::SeqCst);
}

/// Check if debug dump mode is enabled
pub fn is_debug_dump_enabled() -> bool {
    DEBUG_DUMP_ENABLED.load(Ordering::SeqCst)
}

/// Simplified status structure for E2E testing
/// Only includes information needed for E2E validation
#[derive(Debug, Clone, Serialize)]
pub struct DebugStatus {
    pub ports: Vec<DebugPort>,
    pub page: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugPort {
    pub name: String,
    pub enabled: bool,
    pub state: String,
    pub modbus_masters: Vec<DebugModbusMaster>,
    pub modbus_slaves: Vec<DebugModbusSlave>,
    pub log_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugModbusMaster {
    pub station_id: u8,
    pub register_type: String,
    pub start_address: u16,
    pub register_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugModbusSlave {
    pub station_id: u8,
    pub register_type: String,
    pub start_address: u16,
    pub register_count: usize,
}

/// Start a background thread that periodically dumps status to a file
///
/// The file is overwritten (not appended) on each dump to keep file size manageable.
/// Dumps occur every 500ms.
///
/// # Arguments
/// * `output_path` - Path to the output file (e.g., "/tmp/tui_e2e.log" or "/tmp/cli_e2e_vcom1.log")
/// * `shutdown_signal` - Optional Arc<AtomicBool> to signal thread shutdown
///
/// # Returns
/// A JoinHandle to the spawned thread
pub fn start_status_dump_thread(
    output_path: PathBuf,
    shutdown_signal: Option<Arc<AtomicBool>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        log::info!(
            "Started status dump thread, writing to {}",
            output_path.display()
        );

        loop {
            // Check shutdown signal
            if let Some(ref signal) = shutdown_signal {
                if signal.load(Ordering::SeqCst) {
                    log::info!("Status dump thread shutting down");
                    break;
                }
            }

            // Dump status to file
            if let Err(e) = dump_status_to_file(&output_path) {
                log::warn!("Failed to dump status to {}: {}", output_path.display(), e);
            }

            // Sleep for 500ms
            thread::sleep(Duration::from_millis(500));
        }
    })
}

/// Dump the current status to a file (overwrites existing content)
fn dump_status_to_file(path: &PathBuf) -> Result<()> {
    // Read current status and convert to DebugStatus
    let debug_status = read_status(|status| {
        use crate::protocol::status::{
            types::{port::PortState, Page},
            with_port_read,
        };

        let mut ports = Vec::new();

        for port_name in &status.ports.order {
            if let Some(port_arc) = status.ports.map.get(port_name) {
                if let Some(Ok(port_data)) = with_port_read(port_arc, |port| {
                    let enabled = matches!(port.state, PortState::OccupiedByThis { .. });
                    let state = match &port.state {
                        PortState::Free => "Free".to_string(),
                        PortState::OccupiedByThis { .. } => "OccupiedByThis".to_string(),
                        PortState::OccupiedByOther => "OccupiedByOther".to_string(),
                    };

                    // Extract stations from config
                    let mut modbus_masters = Vec::new();
                    let mut modbus_slaves = Vec::new();

                    use crate::protocol::status::types::port::PortConfig;
                    let PortConfig::Modbus { mode, stations } = &port.config;
                    for station in stations {
                        let item = if mode.is_master() {
                            modbus_masters.push(DebugModbusMaster {
                                station_id: station.station_id,
                                register_type: format!("{:?}", station.register_mode),
                                start_address: station.register_address,
                                register_count: station.register_length as usize,
                            });
                            true
                        } else {
                            modbus_slaves.push(DebugModbusSlave {
                                station_id: station.station_id,
                                register_type: format!("{:?}", station.register_mode),
                                start_address: station.register_address,
                                register_count: station.register_length as usize,
                            });
                            true
                        };
                        let _ = item; // Suppress unused warning
                    }

                    Ok::<DebugPort, anyhow::Error>(DebugPort {
                        name: port.port_name.clone(),
                        enabled,
                        state,
                        modbus_masters,
                        modbus_slaves,
                        log_count: port.logs.len(),
                    })
                }) {
                    ports.push(port_data);
                }
            }
        }

        let page = match &status.page {
            Page::Entry { .. } => "Entry",
            Page::ConfigPanel { .. } => "ConfigPanel",
            Page::ModbusDashboard { .. } => "ModbusDashboard",
            Page::LogPanel { .. } => "LogPanel",
            Page::About { .. } => "About",
        };

        Ok(DebugStatus {
            ports,
            page: page.to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
        })
    })?;

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&debug_status)?;

    // Write to file (overwrite mode)
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    file.flush()?;

    Ok(())
}
