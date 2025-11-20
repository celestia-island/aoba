//! Daemon mode for TUI
//!
//! This module implements non-interactive daemon mode that loads TUI configuration
//! and runs without the interactive interface. It's designed for transparent port
//! forwarding and other scenarios where TUI configuration is needed but interactive
//! UI is not desired.

use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::{
    core::{
        bus::{Bus, CoreToUi, UiToCore},
        task_manager::spawn_task,
    },
    protocol::status::debug_dump::{enable_debug_dump, start_status_dump_thread},
    tui::status::{
        port::{PortConfig, PortData, PortState, PortStatusIndicator},
        Status, TuiStatus,
    },
};

/// Run TUI in daemon mode (non-interactive)
///
/// This mode loads the configuration from file and automatically starts all configured
/// ports without showing the interactive UI. It's useful for scenarios where TUI configuration
/// (like transparent port forwarding) is needed but interactive interface is not.
///
/// # Arguments
/// - `matches`: Command line arguments
///
/// # Returns
/// - `Ok(())` on successful execution
/// - `Err` if configuration file not found or daemon startup fails
pub async fn start_daemon(matches: &clap::ArgMatches) -> Result<()> {
    log::info!("🤖 Starting TUI in daemon mode (non-interactive)");

    // Determine config file path
    let config_path = if let Some(path) = matches.get_one::<String>("daemon-config") {
        PathBuf::from(path)
    } else {
        // Default to working directory
        std::env::current_dir()
            .context("Failed to get current working directory")?
            .join("aoba_tui_config.json")
    };

    // Check if config file exists
    if !config_path.exists() {
        return Err(anyhow!(
            "Configuration file not found: {}\n\
            \n\
            Daemon mode requires a configuration file. You can:\n\
            1. Run TUI mode first to create and save a configuration\n\
            2. Specify a custom config path with --daemon-config <FILE>",
            config_path.display()
        ));
    }

    log::info!("📂 Loading configuration from: {}", config_path.display());

    // Load configuration
    let persisted_configs = load_config_from_file(&config_path)?;

    if persisted_configs.is_empty() {
        return Err(anyhow!(
            "Configuration file is empty or contains no valid port configurations: {}",
            config_path.display()
        ));
    }

    log::info!(
        "✅ Loaded {} port configuration(s)",
        persisted_configs.len()
    );

    // Initialize global status
    let app = Arc::new(RwLock::new(Status::default()));
    crate::tui::status::init_status(app.clone())?;

    // Set up debug dump if enabled
    let debug_ci_e2e_enabled = matches.get_flag("debug-ci-e2e-test");
    let debug_dump_shutdown = if debug_ci_e2e_enabled {
        log::info!("🔍 Debug CI E2E test mode enabled - starting status dump thread");
        enable_debug_dump();

        let shutdown_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dump_path = PathBuf::from("/tmp/ci_tui_status.json");
        let shutdown_signal_clone = shutdown_signal.clone();

        start_status_dump_thread(
            dump_path,
            Some(shutdown_signal_clone),
            std::sync::Arc::new(|| {
                TuiStatus::from_global_status().and_then(|status| {
                    serde_json::to_string_pretty(&status)
                        .map_err(|e| anyhow!("Failed to serialize TUI status: {e}"))
                })
            }),
        );

        Some(shutdown_signal)
    } else {
        None
    };

    // Populate status with loaded configurations
    let configs_vec: Vec<(String, PortConfig)> = persisted_configs.into_iter().collect();
    let mut autostart_ports: Vec<String> = Vec::new();

    crate::tui::status::write_status(|status| {
        for (port_name, config) in &configs_vec {
            if !status.ports.order.contains(port_name) {
                status.ports.order.push(port_name.clone());
            }

            if !status.ports.map.contains_key(port_name) {
                use crate::protocol::status::types::port::PortType;
                let port_data = PortData {
                    port_name: port_name.clone(),
                    port_type: PortType::detect(port_name),
                    ..PortData::default()
                };
                status.ports.map.insert(port_name.clone(), port_data);
            }

            if let Some(port) = status.ports.map.get_mut(port_name) {
                port.config = config.clone();
                port.config_modified = false;
                port.state = PortState::Free;
                port.subprocess_info = None;
                port.status_indicator = PortStatusIndicator::NotStarted;
                log::info!("✅ Loaded configuration for port: {port_name}");
            }

            // Mark for auto-start if it has stations configured
            match config {
                PortConfig::Modbus { stations, .. } if !stations.is_empty() => {
                    autostart_ports.push(port_name.clone());
                }
                _ => {}
            }
        }
        Ok(())
    })?;

    autostart_ports.sort();
    autostart_ports.dedup();

    log::info!(
        "🚀 Will auto-start {} port(s): {:?}",
        autostart_ports.len(),
        autostart_ports
    );

    // Set up communication channels
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>();
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>();
    let _bus = Bus::new(core_rx.clone(), ui_tx.clone());

    let (thr_tx, _thr_rx) = flume::unbounded::<Result<()>>();
    let (input_kill_tx, _input_kill_rx) = flume::bounded::<()>(1);

    // Start core thread
    let core_task = spawn_task({
        let core_tx = core_tx.clone();
        let thr_tx = thr_tx.clone();
        let ui_rx = ui_rx.clone();
        let input_kill_tx = input_kill_tx.clone();

        async move {
            let res = crate::tui::runtime::run_core_thread(ui_rx, core_tx, input_kill_tx).await;
            thr_tx.send(res)?;
            Ok(())
        }
    });

    // Auto-start all configured ports
    for port_name in &autostart_ports {
        if let Err(err) = ui_tx.send(UiToCore::ToggleRuntime(port_name.clone())) {
            log::warn!("⚠️ Failed to auto-start CLI subprocess for {port_name}: {err}");
        } else {
            log::info!("🚀 Auto-start requested for port {port_name}");
        }
    }

    log::info!("🤖 Daemon mode running. Press Ctrl+C to stop.");

    // Wait for core thread to finish (on Ctrl+C or error)
    let result = core_task.await?;

    if let Some(shutdown_signal) = debug_dump_shutdown {
        shutdown_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("🔍 Debug dump thread shutdown signal sent");
    }

    result
}

/// Load port configurations from a JSON file
fn load_config_from_file(path: &PathBuf) -> Result<HashMap<String, PortConfig>> {
    use crate::core::persistence::{PersistedPortConfig, SerializablePortConfig};
    use crate::protocol::status::types::modbus::{
        ModbusConnectionMode, ModbusMasterDataSource, ModbusRegisterItem, RegisterMode,
    };

    let json = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config from {}", path.display()))?;

    let persisted: Vec<PersistedPortConfig> =
        serde_json::from_str(&json).context("Failed to parse configuration file")?;

    let mut configs = HashMap::new();

    for p in persisted {
        let config = match p.config {
            SerializablePortConfig::Modbus {
                mode,
                master_source,
                stations,
            } => {
                let mode_enum = if mode == "Master" {
                    ModbusConnectionMode::default_master()
                } else {
                    ModbusConnectionMode::default_slave()
                };

                let master_source_enum = if mode_enum.is_master() {
                    master_source
                        .and_then(|src| {
                            let crate::core::persistence::SerializableMasterSource { kind, value } =
                                src;
                            match kind.as_str() {
                                "mqtt" => Some(ModbusMasterDataSource::MqttServer {
                                    url: value.unwrap_or_default(),
                                }),
                                "http" => Some(ModbusMasterDataSource::HttpServer {
                                    port: value.and_then(|v| v.parse().ok()).unwrap_or(8080),
                                }),
                                "ipc" => Some(ModbusMasterDataSource::IpcPipe {
                                    path: value.unwrap_or_default(),
                                }),
                                "port_forwarding" => Some(ModbusMasterDataSource::PortForwarding {
                                    source_port: value.unwrap_or_default(),
                                }),
                                _ => None,
                            }
                        })
                        .unwrap_or_default()
                } else {
                    ModbusMasterDataSource::default()
                };

                let register_items: Vec<ModbusRegisterItem> = stations
                    .iter()
                    .map(|s| {
                        let register_mode = match s.register_mode.as_str() {
                            "Holding" => RegisterMode::Holding,
                            "Input" => RegisterMode::Input,
                            "Coils" => RegisterMode::Coils,
                            "DiscreteInputs" => RegisterMode::DiscreteInputs,
                            _ => RegisterMode::Holding, // Default fallback
                        };

                        ModbusRegisterItem {
                            station_id: s.station_id,
                            register_mode,
                            register_address: s.register_address,
                            register_length: s.register_length,
                            last_values: s.last_values.clone(),
                            req_success: 0,
                            req_total: 0,
                            next_poll_at: std::time::Instant::now(),
                            last_request_time: None,
                            last_response_time: None,
                            pending_requests: Vec::new(),
                            pending_writes: std::collections::HashMap::new(),
                        }
                    })
                    .collect();

                PortConfig::Modbus {
                    mode: mode_enum,
                    master_source: master_source_enum,
                    stations: register_items,
                }
            }
        };

        configs.insert(p.name, config);
    }

    Ok(configs)
}
