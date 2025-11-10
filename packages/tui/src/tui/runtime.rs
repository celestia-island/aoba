use anyhow::{anyhow, Result};
use chrono::Local;
use parking_lot::RwLock;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc, thread, time::Duration};

use crate::tui::{
    cli_data::initialize_cli_data_source,
    ipc::handle_cli_ipc_message,
    logs::*,
    status::{
        port::{PortConfig, PortData, PortState, PortStatusIndicator, PortSubprocessInfo},
        {self as types, Status, TuiStatus},
    },
    subprocess::{CliSubprocessConfig, SubprocessManager},
    utils::bus::{Bus, CoreToUi, UiToCore},
};
use aoba_protocol::status::debug_dump::{enable_debug_dump, start_status_dump_thread};

pub async fn start(matches: &clap::ArgMatches) -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");

    let no_cache = matches.get_flag("no-config-cache");
    crate::tui::persistence::set_no_cache(no_cache);

    let screen_capture_mode = matches.get_flag("debug-screen-capture");
    if screen_capture_mode {
        log::info!("📸 Screen capture mode enabled - will render once and exit");
        return crate::tui::rendering::run_screen_capture_mode();
    }

    if let Some(channel_id) = matches.get_one::<String>("debug-ci") {
        log::info!("🔧 Debug CI mode enabled - starting with IPC: {channel_id}");
        return crate::tui::rendering::start_with_ipc(matches, channel_id).await;
    }

    let app = Arc::new(RwLock::new(Status::default()));
    crate::tui::status::init_status(app.clone())?;

    let debug_ci_e2e_enabled = matches.get_flag("debug-ci-e2e-test");
    let debug_dump_shutdown = if debug_ci_e2e_enabled {
        log::info!("🔍 Debug CI E2E test mode enabled - starting status dump thread");
        enable_debug_dump();

        let shutdown_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dump_path = PathBuf::from("/tmp/ci_tui_status.json");
        let shutdown_signal_clone = shutdown_signal.clone();

        start_status_dump_thread(dump_path, Some(shutdown_signal_clone), || {
            TuiStatus::from_global_status().and_then(|status| {
                serde_json::to_string_pretty(&status)
                    .map_err(|e| anyhow!("Failed to serialize TUI status: {e}"))
            })
        });

        Some(shutdown_signal)
    } else {
        None
    };

    let mut autostart_ports: Vec<String> = Vec::new();

    if let Ok(persisted_configs) = crate::tui::persistence::load_port_configs() {
        if !persisted_configs.is_empty() {
            let configs_vec: Vec<(String, PortConfig)> = persisted_configs.into_iter().collect();

            autostart_ports = configs_vec
                .iter()
                .filter_map(|(name, config)| match config {
                    PortConfig::Modbus { stations, .. } if !stations.is_empty() => {
                        Some(name.clone())
                    }
                    _ => None,
                })
                .collect();

            autostart_ports.sort();
            autostart_ports.dedup();

            crate::tui::status::write_status(|status| {
                for (port_name, config) in &configs_vec {
                    if !status.ports.order.contains(port_name) {
                        status.ports.order.push(port_name.clone());
                    }

                    if !status.ports.map.contains_key(port_name) {
                        let mut port_data = PortData {
                            port_name: port_name.clone(),
                            ..PortData::default()
                        };
                        port_data.port_type = "Cached".to_string();
                        status.ports.map.insert(port_name.clone(), port_data);
                    }

                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        port.config = config.clone();
                        port.config_modified = false;
                        port.state = PortState::Free;
                        port.subprocess_info = None;
                        port.status_indicator = PortStatusIndicator::NotStarted;
                        log::info!("✅ Restored cached configuration for port: {port_name}");
                    }
                }
                Ok(())
            })?;

            log::info!("📂 Restored {} port configuration(s)", configs_vec.len());
        }
    }

    if std::env::var("AOBA_TUI_FORCE_ERROR").is_ok() {
        crate::tui::status::write_status(|g| {
            crate::tui::ui::components::error_msg::ui_error_set(
                g,
                Some((
                    "demo forced error: AOBA_TUI_FORCE_ERROR".to_string(),
                    Local::now(),
                )),
            );
            Ok(())
        })?;
    }

    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>();
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>();
    let bus = Bus::new(core_rx.clone(), ui_tx.clone());

    let (thr_tx, thr_rx) = flume::unbounded::<Result<()>>();

    let (input_kill_tx, input_kill_rx) = flume::bounded::<()>(1);

    let core_handle = thread::spawn({
        let core_tx = core_tx.clone();
        let thr_tx = thr_tx.clone();
        let ui_rx = ui_rx.clone();
        let input_kill_tx = input_kill_tx.clone();

        move || thr_tx.send(run_core_thread(ui_rx, core_tx, input_kill_tx))
    });

    let input_handle = thread::spawn({
        let bus = bus.clone();
        move || crate::tui::input::run_input_thread(bus, input_kill_rx)
    });

    let render_handle =
        thread::spawn(move || crate::tui::rendering::run_rendering_loop(bus, thr_rx));

    // NOTE: Initial port scan will be triggered automatically by core thread's first loop iteration
    // since last_scan is initialized to (now - scan_interval), making it immediately eligible for scanning.
    // This approach avoids race conditions between manual RescanPorts message and automatic scanning.
    log::info!("🔍 Core thread will perform initial port scan on first iteration");

    for port_name in &autostart_ports {
        if let Err(err) = ui_tx.send(UiToCore::ToggleRuntime(port_name.clone())) {
            log::warn!("⚠️ Failed to auto-start CLI subprocess for {port_name}: {err}");
        } else {
            log::info!("🚀 Auto-start requested for cached port {port_name}");
        }
    }

    core_handle
        .join()
        .map_err(|err| anyhow!("Failed to join core thread: {err:?}"))??;
    render_handle
        .join()
        .map_err(|err| anyhow!("Failed to join render thread: {err:?}"))??;

    input_kill_tx.send(())?;
    input_handle
        .join()
        .map_err(|err| anyhow!("Failed to join input thread: {err:?}"))??;

    if let Some(shutdown_signal) = debug_dump_shutdown {
        shutdown_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("🔍 Debug dump thread shutdown signal sent");
    }

    Ok(())
}

pub fn run_core_thread(
    ui_rx: flume::Receiver<UiToCore>,
    core_tx: flume::Sender<CoreToUi>,
    input_kill_tx: flume::Sender<()>,
) -> Result<()> {
    let mut polling_enabled = true;
    let scan_interval = Duration::from_secs(30);
    let mut last_scan = std::time::Instant::now() - scan_interval;
    let mut scan_in_progress = false;

    let _last_modbus_run = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut subprocess_manager = SubprocessManager::new();
    loop {
        let msg_count_before = ui_rx.len();
        let mut msg_count_processed = 0;
        while let Ok(msg) = ui_rx.try_recv() {
            msg_count_processed += 1;
            let msg_name = match &msg {
                UiToCore::Quit => "Quit".to_string(),
                UiToCore::Refresh => "Refresh".to_string(),
                UiToCore::RescanPorts => "RescanPorts".to_string(),
                UiToCore::PausePolling => "PausePolling".to_string(),
                UiToCore::ResumePolling => "ResumePolling".to_string(),
                UiToCore::ToggleRuntime(port) => format!("ToggleRuntime({port})"),
                UiToCore::RestartRuntime(port) => format!("RestartRuntime({port})"),
                UiToCore::SendRegisterUpdate {
                    port_name,
                    station_id,
                    start_address,
                    values,
                    ..
                } => {
                    format!("SendRegisterUpdate(port={port_name}, station={station_id}, addr={start_address}, values={values:?})")
                }
            };
            log::info!("🔵 Core thread received message: {msg_name}");
            match msg {
                UiToCore::Quit => {
                    log::info!("Received quit signal");
                    subprocess_manager.shutdown_all();
                    if let Err(err) = crate::tui::status::write_status(|status| {
                        for port in status.ports.map.values_mut() {
                            port.state = PortState::Free;
                            port.subprocess_info = None;
                            port.status_indicator = PortStatusIndicator::NotStarted;
                        }
                        Ok(())
                    }) {
                        log::warn!("Failed to reset port statuses while quitting: {err}");
                    }
                    if let Err(err) = input_kill_tx.send(()) {
                        log::warn!("Failed to send input kill signal: {err}");
                    }
                    core_tx
                        .send(CoreToUi::Quit)
                        .map_err(|err| anyhow!("Failed to send Quit to UI core: {err}"))?;
                    return Ok(());
                }
                UiToCore::Refresh => {
                    core_tx.send(CoreToUi::Refreshed).map_err(|err| {
                        anyhow!("Failed to send Refreshed event to UI core: {err}")
                    })?;
                    if let Err(err) = crate::tui::status_utils::log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::RescanPorts => {
                    if crate::tui::utils::scan::scan_ports(&core_tx, &mut scan_in_progress)? {
                        last_scan = std::time::Instant::now();
                    }
                }
                UiToCore::PausePolling => {
                    polling_enabled = false;
                    if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                        log::warn!("ToggleRuntime: failed to send Refreshed: {err}");
                    }
                    if let Err(err) = crate::tui::status_utils::log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::ResumePolling => {
                    polling_enabled = true;
                    core_tx.send(CoreToUi::Refreshed).map_err(|err| {
                        anyhow!("Failed to send Refreshed event to UI core: {err}")
                    })?;
                    if let Err(err) = crate::tui::status_utils::log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::ToggleRuntime(port_name) => {
                    log::info!("ToggleRuntime requested for {port_name}");
                    let was_running = stop_runtime(
                        "ToggleRuntime",
                        &port_name,
                        &mut subprocess_manager,
                        &core_tx,
                    )?;
                    if was_running {
                        continue;
                    }
                    start_runtime(
                        "ToggleRuntime",
                        &port_name,
                        &mut subprocess_manager,
                        &core_tx,
                    )?;
                }
                UiToCore::RestartRuntime(port_name) => {
                    log::info!("RestartRuntime requested for {port_name}");
                    restart_runtime(
                        "RestartRuntime",
                        &port_name,
                        &mut subprocess_manager,
                        &core_tx,
                    )?;
                }
                UiToCore::SendRegisterUpdate {
                    port_name,
                    station_id,
                    register_type,
                    start_address,
                    values,
                } => {
                    log::info!("🔵 SendRegisterUpdate requested for {port_name}: station={station_id}, type={register_type}, addr={start_address}, values={values:?}");
                    log::debug!(
                        "📡 Sending full stations update for {port_name} to ensure synchronization"
                    );
                    if let Err(err) = subprocess_manager.send_stations_update_for_port(&port_name) {
                        log::warn!("❌ Failed to send stations update for {port_name}: {err}");
                    } else {
                        log::debug!("✅ Sent full stations update for {port_name}");
                    }
                }
            }
        }

        if msg_count_before > 0 || msg_count_processed > 0 {
            log::info!("📊 Core thread: queue had {msg_count_before} messages, processed {msg_count_processed}");
        }

        let dead_processes = subprocess_manager.reap_dead_processes();
        if !dead_processes.is_empty() {
            let mut cleanup_paths: HashMap<String, Option<String>> = HashMap::new();
            crate::tui::status::write_status(|status| {
                for (port_name, _) in &dead_processes {
                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        if port.state.is_occupied_by_this() {
                            if let Some(info) = &port.subprocess_info {
                                cleanup_paths
                                    .insert(port_name.clone(), info.data_source_path.clone());
                            }
                            port.state = PortState::Free;
                            port.subprocess_info = None;
                            port.status_indicator = types::port::PortStatusIndicator::NotStarted;
                        }
                    }
                }
                Ok(())
            })?;

            for (port_name, exit_status) in dead_processes {
                if let Some(Some(path)) = cleanup_paths.remove(&port_name) {
                    if let Err(err) = fs::remove_file(&path) {
                        log::debug!("cleanup: failed to remove data source {path}: {err}");
                    }
                }
                append_subprocess_exited_log(&port_name, exit_status);
                if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                    log::warn!("Failed to send Refreshed after CLI exit for {port_name}: {err}");
                }
            }
        }

        for (port_name, message) in subprocess_manager.poll_ipc_messages() {
            if let Err(err) = handle_cli_ipc_message(port_name.as_str(), message) {
                log::warn!("Failed to handle IPC message for {port_name}: {err}");
            }
        }

        if polling_enabled
            && last_scan.elapsed() >= scan_interval
            && crate::tui::utils::scan::scan_ports(&core_tx, &mut scan_in_progress)?
        {
            last_scan = std::time::Instant::now();
        }

        crate::tui::status::write_status(|status| {
            status.temporarily.busy.spinner_frame =
                status.temporarily.busy.spinner_frame.wrapping_add(1);
            Ok(())
        })?;

        // Check and auto-transition temporary statuses (AppliedSuccess, StartupFailed)
        // Pass core_tx to trigger immediate UI refresh when status changes
        crate::tui::status_utils::check_and_update_temporary_statuses(Some(&core_tx))?;

        core_tx
            .send(CoreToUi::Tick)
            .map_err(|err| anyhow!("failed to send Tick: {err}"))?;
        thread::sleep(Duration::from_millis(50));
    }
}

fn restart_runtime(
    label: &str,
    port_name: &str,
    subprocess_manager: &mut SubprocessManager,
    core_tx: &flume::Sender<CoreToUi>,
) -> Result<()> {
    // Set status to Restarting before stopping the process
    crate::tui::status::write_status(|status| {
        if let Some(port) = status.ports.map.get_mut(port_name) {
            port.status_indicator = types::port::PortStatusIndicator::Restarting;
            port.config_modified = false; // Clear the modified flag since we're restarting
        }
        Ok(())
    })?;

    if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
        log::warn!("{label}: failed to send Refreshed after setting Restarting: {err}");
    }

    // Stop the subprocess (but don't clear the state)
    let subprocess_info_opt = crate::tui::status::read_status(|status| {
        if let Some(port) = status.ports.map.get(port_name) {
            return Ok(port.subprocess_info.clone());
        }
        Ok(None)
    })?;

    if let Some(info) = subprocess_info_opt {
        if let Err(err) = subprocess_manager.stop_subprocess(port_name) {
            log::warn!("{label}: failed to stop CLI subprocess for {port_name}: {err}");
        }

        if let Some(path) = info.data_source_path.clone() {
            if let Err(err) = fs::remove_file(&path) {
                log::debug!("{label}: failed to remove data source {path}: {err}");
            }
        }

        // Clear subprocess info but KEEP the port state and Modbus config
        crate::tui::status::write_status(|status| {
            if let Some(port) = status.ports.map.get_mut(port_name) {
                port.subprocess_info = None;
                // Keep port.state as OccupiedByThis
                // Keep port.config and all Modbus stations
                // Keep status_indicator as Restarting
            }
            Ok(())
        })?;

        append_subprocess_stopped_log(port_name, Some("重启中 - 停止旧进程".to_string()));
    }

    // Start the new subprocess
    start_runtime(label, port_name, subprocess_manager, core_tx)?;

    Ok(())
}

fn stop_runtime(
    label: &str,
    port_name: &str,
    subprocess_manager: &mut SubprocessManager,
    core_tx: &flume::Sender<CoreToUi>,
) -> Result<bool> {
    let subprocess_info_opt = crate::tui::status::read_status(|status| {
        if let Some(port) = status.ports.map.get(port_name) {
            return Ok(port.subprocess_info.clone());
        }
        Ok(None)
    })?;

    if let Some(info) = subprocess_info_opt {
        if let Err(err) = subprocess_manager.stop_subprocess(port_name) {
            log::warn!("{label}: failed to stop CLI subprocess for {port_name}: {err}");
        }

        if let Some(path) = info.data_source_path.clone() {
            if let Err(err) = fs::remove_file(&path) {
                log::debug!("{label}: failed to remove data source {path}: {err}");
            }
        }

        crate::tui::status::write_status(|status| {
            if let Some(port) = status.ports.map.get_mut(port_name) {
                port.state = PortState::Free;
                port.subprocess_info = None;
                port.status_indicator = types::port::PortStatusIndicator::NotStarted;
            }
            Ok(())
        })?;

        append_subprocess_stopped_log(
            port_name,
            Some(
                crate::i18n::lang()
                    .tabs
                    .log
                    .subprocess_stopped_reason_tui
                    .clone(),
            ),
        );

        if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
            log::warn!("{label}: failed to send Refreshed: {err}");
        }
        if let Err(err) = crate::tui::status_utils::log_state_snapshot() {
            log::warn!("Failed to log state snapshot: {err}");
        }

        return Ok(true);
    }

    Ok(false)
}

fn start_runtime(
    label: &str,
    port_name: &str,
    subprocess_manager: &mut SubprocessManager,
    core_tx: &flume::Sender<CoreToUi>,
) -> Result<bool> {
    let cli_inputs = crate::tui::status::read_status(|status| {
        if let Some(port) = status.ports.map.get(port_name) {
            let types::port::PortConfig::Modbus { mode, stations } = &port.config;
            log::info!(
                "{label}({port_name}): checking CLI inputs - mode={}, station_count={}",
                if mode.is_master() { "Master" } else { "Slave" },
                stations.len()
            );
            if !stations.is_empty() {
                let baud = port.serial_config.baud;
                let request_interval_ms = port.serial_config.request_interval_ms;
                let timeout_ms = port.serial_config.timeout_ms;
                log::info!(
                    "{label}({port_name}): found {} station(s) - will attempt CLI subprocess (baud={}, interval={}ms, timeout={}ms)",
                    stations.len(),
                    baud,
                    request_interval_ms,
                    timeout_ms
                );
                return Ok(Some((
                    mode.clone(),
                    stations.clone(),
                    baud,
                    request_interval_ms,
                    timeout_ms,
                )));
            }
            log::info!("{label}({port_name}): no station configured - nothing to do");
        }
        Ok(None)
    })?;

    let mut cli_started = false;

    if let Some((mode, stations, baud_rate, request_interval_ms, timeout_ms)) = cli_inputs {
        match mode {
            types::modbus::ModbusConnectionMode::Slave { .. } => {
                let station = &stations[0];
                log::info!(
                    "{label}: attempting to spawn CLI subprocess (SlavePoll) for {port_name}"
                );

                let cli_config = CliSubprocessConfig {
                    port_name: port_name.to_string(),
                    mode: aoba_cli::status::CliMode::SlavePoll,
                    station_id: station.station_id,
                    register_address: station.register_address,
                    register_length: station.register_length,
                    register_mode: crate::tui::cli_data::register_mode_to_cli_arg(
                        station.register_mode,
                    )
                    .to_string(),
                    baud_rate,
                    request_interval_ms,
                    timeout_ms,
                    data_source: None,
                };

                match subprocess_manager.start_subprocess(cli_config) {
                    Ok(()) => {
                        if let Some(snapshot) = subprocess_manager.snapshot(port_name) {
                            let subprocess_info = PortSubprocessInfo {
                                mode: crate::tui::cli_data::cli_mode_to_port_mode(&snapshot.mode),
                                ipc_socket_name: snapshot.ipc_socket_name.clone(),
                                pid: snapshot.pid,
                                data_source_path: None,
                            };

                            crate::tui::status::write_status(|status| {
                                if let Some(port) = status.ports.map.get_mut(port_name) {
                                    port.state = PortState::OccupiedByThis;
                                    port.subprocess_info = Some(subprocess_info.clone());
                                    port.status_indicator =
                                        types::port::PortStatusIndicator::Running;
                                    port.config_modified = false; // Clear modified flag when starting
                                }
                                Ok(())
                            })?;

                            append_subprocess_spawned_log(port_name, &snapshot.mode, snapshot.pid);
                            cli_started = true;

                            log::info!("📡 Sending initial stations configuration to CLI subprocess for {port_name}");
                            let mut stations_sent = false;
                            for attempt in 1..=10 {
                                match subprocess_manager.send_stations_update_for_port(port_name) {
                                    Ok(()) => {
                                        stations_sent = true;
                                        break;
                                    }
                                    Err(_err) if attempt < 10 => {
                                        thread::sleep(Duration::from_millis(200));
                                    }
                                    Err(err) => {
                                        log::warn!("⚠️ Failed to send initial stations update for {port_name} after {attempt} attempts: {err}");
                                    }
                                }
                            }
                            if !stations_sent {
                                log::error!("❌ Could not send initial stations configuration to {port_name}");
                            }
                        }
                    }
                    Err(err) => {
                        let err_text = err.to_string();
                        let msg =
                            format!("Failed to start CLI subprocess for {port_name}: {err_text}");
                        append_lifecycle_log(
                            port_name,
                            crate::tui::status::port::PortLifecyclePhase::Failed,
                            Some(err_text.clone()),
                        );
                        crate::tui::status::write_status(|status| {
                            status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                                message: msg.clone(),
                                timestamp: Local::now(),
                            });
                            Ok(())
                        })?;
                    }
                }
            }
            types::modbus::ModbusConnectionMode::Master => {
                log::info!("{label}: attempting to spawn CLI subprocess (MasterProvide) for {port_name} with {} station(s)", stations.len());

                let (data_source_path, merged_station_id, merged_start_addr, merged_length) =
                    initialize_cli_data_source(port_name, &stations)?;

                let cli_config = CliSubprocessConfig {
                    port_name: port_name.to_string(),
                    mode: aoba_cli::status::CliMode::MasterProvide,
                    station_id: merged_station_id as u8,
                    register_address: merged_start_addr,
                    register_length: merged_length,
                    register_mode: crate::tui::cli_data::register_mode_to_cli_arg(
                        stations[0].register_mode,
                    )
                    .to_string(),
                    baud_rate,
                    request_interval_ms,
                    timeout_ms,
                    data_source: Some(format!("file:{}", data_source_path.to_string_lossy())),
                };

                match subprocess_manager.start_subprocess(cli_config) {
                    Ok(()) => {
                        if let Some(snapshot) = subprocess_manager.snapshot(port_name) {
                            let subprocess_info = PortSubprocessInfo {
                                mode: crate::tui::cli_data::cli_mode_to_port_mode(&snapshot.mode),
                                ipc_socket_name: snapshot.ipc_socket_name.clone(),
                                pid: snapshot.pid,
                                data_source_path: Some(
                                    data_source_path.to_string_lossy().to_string(),
                                ),
                            };
                            crate::tui::status::write_status(|status| {
                                if let Some(port) = status.ports.map.get_mut(port_name) {
                                    port.state = PortState::OccupiedByThis;
                                    port.subprocess_info = Some(subprocess_info.clone());
                                    port.status_indicator =
                                        types::port::PortStatusIndicator::Running;
                                    port.config_modified = false; // Clear modified flag when starting
                                }
                                Ok(())
                            })?;

                            append_subprocess_spawned_log(port_name, &snapshot.mode, snapshot.pid);
                            cli_started = true;

                            log::info!("📡 Sending initial stations configuration to CLI subprocess for {port_name}");
                            let mut stations_sent = false;
                            for attempt in 1..=10 {
                                match subprocess_manager.send_stations_update_for_port(port_name) {
                                    Ok(()) => {
                                        stations_sent = true;
                                        break;
                                    }
                                    Err(_err) if attempt < 10 => {
                                        thread::sleep(Duration::from_millis(200));
                                    }
                                    Err(err) => {
                                        log::warn!("⚠️ Failed to send initial stations update for {port_name} after {attempt} attempts: {err}");
                                    }
                                }
                            }
                            if !stations_sent {
                                log::error!("❌ Could not send initial stations configuration to {port_name}");
                            }
                        }
                    }
                    Err(err) => {
                        let err_text = err.to_string();
                        let msg =
                            format!("Failed to start CLI subprocess for {port_name}: {err_text}");
                        append_lifecycle_log(
                            port_name,
                            crate::tui::status::port::PortLifecyclePhase::Failed,
                            Some(err_text.clone()),
                        );

                        crate::tui::status::write_status(|status| {
                            if let Some(port) = status.ports.map.get_mut(port_name) {
                                port.status_indicator =
                                    types::port::PortStatusIndicator::StartupFailed {
                                        error_message: err_text.clone(),
                                        timestamp: Local::now(),
                                    };
                            }
                            status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                                message: msg.clone(),
                                timestamp: Local::now(),
                            });
                            Ok(())
                        })?;

                        if let Err(remove_err) = fs::remove_file(&data_source_path) {
                            log::debug!(
                                "Cleanup of data source {} failed: {remove_err}",
                                data_source_path.to_string_lossy()
                            );
                        }
                    }
                }
            }
        }
    }

    if !cli_started {
        log::warn!("{label}: CLI subprocess failed to start for {port_name}, port remains Free");
    }

    if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
        log::warn!("{label}: failed to send Refreshed: {err}");
    }
    if let Err(err) = crate::tui::status_utils::log_state_snapshot() {
        log::warn!("Failed to log state snapshot: {err}");
    }

    Ok(cli_started)
}
