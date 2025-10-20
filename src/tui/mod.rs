pub mod status;
pub mod input;
pub mod persistence;
pub mod subprocess;
pub mod ui;
pub mod utils;

// Re-export Page for convenience since it's used throughout TUI code
pub use status::Page;

use anyhow::{anyhow, Result};
use chrono::Local;
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    thread,
    time::Duration,
};

use ratatui::{backend::CrosstermBackend, layout::*, prelude::*};

use crate::{
    protocol::{
        ipc::IpcMessage,
        status::{
            types::{
                self,
                modbus::RegisterMode,
                port::{
                    PortLogEntry, PortState, PortSubprocessInfo, PortSubprocessMode,
                },
            },
        },
    },
    tui::{
        status::Status,
        subprocess::{CliMode, CliSubprocessConfig, SubprocessManager},
        ui::components::error_msg::ui_error_set,
        utils::bus::{Bus, CoreToUi, UiToCore},
    },
};

/// Create a stable data source file path for a specific station on a port.
///
/// The path is deterministic based on port name, station ID, register mode, and address,
/// allowing multiple Masters on the same port to maintain separate data files without
/// conflicts. The format is: `aoba_cli_{port}_s{station_id}_t{type:02}_a{addr:04X}.jsonl`
///
/// Example: `/tmp/aoba_cli__tmp_vcom1_s1_t03_a0000.jsonl`
fn create_cli_data_source_path(
    port_name: &str,
    station_id: u8,
    register_mode: RegisterMode,
    start_address: u16,
) -> PathBuf {
    let sanitized: String = port_name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => c,
            _ => '_',
        })
        .collect();
    let fallback = if sanitized.is_empty() {
        "port".to_string()
    } else {
        sanitized
    };

    // Convert register mode to 2-digit type code (01-04)
    let type_code = match register_mode {
        RegisterMode::Coils => 1,
        RegisterMode::DiscreteInputs => 2,
        RegisterMode::Holding => 3,
        RegisterMode::Input => 4,
    };

    let mut path = std::env::temp_dir();
    path.push(format!(
        "aoba_cli_{fallback}_s{station_id}_t{type_code:02}_a{start_address:04X}.jsonl"
    ));
    path
}

fn append_port_log(port_name: &str, raw: String) {
    let entry = PortLogEntry {
        when: Local::now(),
        raw,
        parsed: None,
    };

    if let Err(err) = self::status::write_status(|status| {
        if let Some(port) = status.ports.map.get_mut(port_name) {
            port.logs.push(entry.clone());
            if port.logs.len() > 1000 {
                let excess = port.logs.len() - 1000;
                port.logs.drain(0..excess);
            }
        }
        Ok(())
    }) {
        log::warn!("append_port_log: failed to persist log entry for {port_name}: {err}");
    }
}

fn register_mode_to_cli_arg(mode: types::modbus::RegisterMode) -> &'static str {
    use types::modbus::RegisterMode;

    match mode {
        RegisterMode::Coils => "coils",
        RegisterMode::DiscreteInputs => "discrete",
        RegisterMode::Holding => "holding",
        RegisterMode::Input => "input",
    }
}

fn cli_mode_to_port_mode(mode: &CliMode) -> PortSubprocessMode {
    match mode {
        CliMode::SlaveListen => PortSubprocessMode::SlaveListen,
        CliMode::SlavePoll => PortSubprocessMode::SlavePoll,
        CliMode::MasterProvide => PortSubprocessMode::MasterProvide,
    }
}

fn station_values_for_cli(station: &types::modbus::ModbusRegisterItem) -> Vec<u16> {
    let target_len = station.register_length as usize;
    if target_len == 0 {
        return Vec::new();
    }

    let mut values = station.last_values.clone();
    values.resize(target_len, 0);
    values
}

/// Initialize CLI data source for Master mode by merging all stations' data.
///
/// For a Master port with multiple stations (address ranges), this function:
/// 1. Collects all stations with the same station_id and register_mode
/// 2. Determines the overall address range (min to max)
/// 3. Merges all stations' data into a continuous array
/// 4. Writes the merged data to a single data file
///
/// The CLI subprocess will then serve this entire address range.
fn initialize_cli_data_source(
    port_name: &str,
    stations: &[types::modbus::ModbusRegisterItem],
) -> Result<(PathBuf, u16, u16, u16)> {
    if stations.is_empty() {
        return Err(anyhow::anyhow!(
            "No stations provided for data source initialization"
        ));
    }

    // Use first station's metadata as reference
    let first = &stations[0];
    let station_id = first.station_id;
    let register_mode = first.register_mode;

    // Find min and max addresses across all stations
    let mut min_addr = u16::MAX;
    let mut max_addr = 0u16;

    for station in stations {
        if station.station_id != station_id {
            log::warn!(
                "initialize_cli_data_source: skipping station with different ID {} (expected {})",
                station.station_id,
                station_id
            );
            continue;
        }
        if station.register_mode != register_mode {
            log::warn!(
                "initialize_cli_data_source: skipping station with different register mode (expected {:?})",
                register_mode
            );
            continue;
        }

        let start = station.register_address;
        let end = start + station.register_length;

        if start < min_addr {
            min_addr = start;
        }
        if end > max_addr {
            max_addr = end;
        }
    }

    let total_length = max_addr - min_addr;
    log::info!(
        "initialize_cli_data_source: merging {} stations for {port_name}, station_id={}, type={:?}, address range: 0x{:04X}-0x{:04X} (length={})",
        stations.len(),
        station_id,
        register_mode,
        min_addr,
        max_addr,
        total_length
    );

    // Create merged data array
    let mut merged_data = vec![0u16; total_length as usize];

    // Fill in data from each station
    for station in stations {
        if station.station_id != station_id || station.register_mode != register_mode {
            continue;
        }

        let start_offset = (station.register_address - min_addr) as usize;
        let station_values = station_values_for_cli(station);

        log::debug!(
            "  Merging station at 0x{:04X}, length={}, into offset {}",
            station.register_address,
            station_values.len(),
            start_offset
        );

        for (i, &value) in station_values.iter().enumerate() {
            let target_idx = start_offset + i;
            if target_idx < merged_data.len() {
                merged_data[target_idx] = value;
            }
        }
    }

    // Create path using first station's info (but covering full range)
    let path = create_cli_data_source_path(port_name, station_id, register_mode, min_addr);

    if let Err(err) = write_cli_data_snapshot(&path, &merged_data, true) {
        log::error!(
            "initialize_cli_data_source: failed to write merged snapshot for {port_name}: {err}"
        );
        return Err(err);
    }

    log::info!(
        "initialize_cli_data_source: created merged data source at {} (station_id={}, addr=0x{:04X}, length={})",
        path.display(),
        station_id,
        min_addr,
        total_length
    );

    Ok((path, station_id as u16, min_addr, total_length))
}

fn write_cli_data_snapshot(path: &PathBuf, values: &[u16], truncate: bool) -> Result<()> {
    let payload = serde_json::json!({ "values": values });
    let serialized = serde_json::to_string(&payload)?;

    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }

    let mut file = options.open(path)?;
    writeln!(file, "{serialized}")?;
    Ok(())
}

// DEPRECATED: This function is no longer used with the new StationsUpdate IPC message format.
// Kept for reference during transition to new Config structure.
// TODO: Remove this function once full state synchronization is implemented.
#[allow(dead_code)]
fn update_cli_data_file(port_name: &str, path: &PathBuf) -> Result<()> {
    // Read current station values and rebuild the merged data file
    let merged_data = self::status::read_status(|status| {
        if let Some(port_entry) = status.ports.map.get(port_name) {
            if let Some(port) = with_port_read(port_entry, |port| {
                let types::port::PortConfig::Modbus { stations, .. } = &port.config;

                if stations.is_empty() {
                    return None;
                }

                // Use first station's metadata as reference
                let first = &stations[0];
                let station_id = first.station_id;
                let register_mode = first.register_mode;

                // Find min and max addresses across all matching stations
                let mut min_addr = u16::MAX;
                let mut max_addr = 0u16;

                for station in stations {
                    if station.station_id != station_id || station.register_mode != register_mode {
                        continue;
                    }

                    let start = station.register_address;
                    let end = start + station.register_length;

                    if start < min_addr {
                        min_addr = start;
                    }
                    if end > max_addr {
                        max_addr = end;
                    }
                }

                let total_length = max_addr - min_addr;

                // Create merged data array
                let mut merged_data = vec![0u16; total_length as usize];

                // Fill in data from each station
                for station in stations {
                    if station.station_id != station_id || station.register_mode != register_mode {
                        continue;
                    }

                    let start_offset = (station.register_address - min_addr) as usize;
                    let station_values = station_values_for_cli(station);

                    for (i, &value) in station_values.iter().enumerate() {
                        let target_idx = start_offset + i;
                        if target_idx < merged_data.len() {
                            merged_data[target_idx] = value;
                        }
                    }
                }

                Some(merged_data)
            }) {
                return Ok(port);
            }
        }
        Ok(None)
    })?;

    if let Some(data) = merged_data {
        write_cli_data_snapshot(path, &data, true)?;
        log::debug!(
            "Updated CLI data file at {} with {} values",
            path.display(),
            data.len()
        );
    }

    Ok(())
}

// DEPRECATED: This function is no longer used with the new StationsUpdate IPC message format.
// Individual RegisterUpdate messages have been replaced by full StationsUpdate synchronization.
// Kept for reference during transition to new Config structure.
// TODO: Remove this function once full state synchronization is implemented.
#[allow(dead_code)]
fn apply_register_update_from_ipc(
    port_name: &str,
    station_id: u8,
    register_mode: RegisterMode,
    start_address: u16,
    values: &[u16],
) -> Result<()> {
    let mut handled = false;
    let mut data_source_path: Option<PathBuf> = None;

    self::status::write_status(|status| {
        if let Some(port) = status.ports.map.get_mut(port_name) {
            let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
            // Find the station that matches station_id, register_mode AND address range
            if let Some(item) = stations.iter_mut().find(|item| {
                if item.station_id != station_id || item.register_mode != register_mode {
                    return false;
                }
                // Check if start_address falls within this station's address range
                let item_start = item.register_address;
                let item_end = item_start + item.register_length;
                start_address >= item_start && start_address < item_end
            }) {
                item.req_total = item.req_total.saturating_add(1);
                item.req_success = item.req_success.saturating_add(1);
                item.last_response_time = Some(std::time::Instant::now());

                let configured_len = item.register_length as usize;
                if configured_len > 0 {
                    if item.last_values.len() != configured_len {
                        item.last_values.resize(configured_len, 0);
                    }

                    if start_address >= item.register_address {
                        let base_index = (start_address - item.register_address) as usize;
                        if base_index < configured_len {
                            let capacity = configured_len - base_index;
                            let limit = std::cmp::min(values.len(), capacity);
                            if values.len() > capacity {
                                log::debug!(
                                    "Register update for {port_name}: truncating values from {} to configured capacity {}",
                                    values.len(),
                                    capacity
                                );
                            }

                            for (offset, value) in values.iter().take(limit).enumerate() {
                                item.last_values[base_index + offset] = *value;
                            }

                            handled = true;
                        } else {
                            log::warn!(
                                "Register update for {port_name}: start address 0x{start_address:04X} outside configured range base=0x{:04X} len=0x{:04X}",
                                item.register_address,
                                item.register_length
                            );
                        }
                    } else {
                        log::warn!(
                            "Register update for {port_name}: start address 0x{start_address:04X} is before configured base 0x{:04X}",
                            item.register_address
                        );
                    }
                }
            } else {
                log::debug!(
                    "Register update for {port_name}: station {station_id} in mode {register_mode:?} not found"
                );
            }

            // Get data source path if available for this port
            if port.state.is_occupied_by_this() {
                if let Some(info) = &port.subprocess_info {
                    if let Some(path_str) = &info.data_source_path {
                        data_source_path = Some(PathBuf::from(path_str));
                    }
                }
            }
        } else {
            log::debug!("Register update for {port_name}: port not found in status map");
        }
        Ok(())
    })?;

    if !handled {
        log::debug!(
            "Register update for {port_name}: no station handled for mode {register_mode:?}"
        );
        return Ok(());
    }

    // If we have a data source path, update the file with latest values from all stations
    if let Some(path) = data_source_path {
        update_cli_data_file(port_name, &path)?;
    }

    Ok(())
}

fn handle_cli_ipc_message(port_name: &str, message: IpcMessage) -> Result<()> {
    match message {
        IpcMessage::PortOpened { .. } => {
            log::info!("CLI[{port_name}]: PortOpened received");
            append_port_log(port_name, "CLI subprocess reported port opened".to_string());
        }
        IpcMessage::PortError { error, .. } => {
            let msg = format!("CLI subprocess error: {error}");
            log::warn!("CLI[{port_name}]: {msg}");
            append_port_log(port_name, msg.clone());
            self::status::write_status(|status| {
                status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                    message: msg.clone(),
                    timestamp: chrono::Local::now(),
                });
                Ok(())
            })?;
        }
        IpcMessage::Shutdown { .. } => {
            log::info!("CLI[{port_name}]: Shutdown received");
            append_port_log(port_name, "CLI subprocess shutting down".to_string());
        }
        IpcMessage::ModbusData {
            direction, data, ..
        } => {
            log::debug!("CLI[{port_name}]: ModbusData {direction} {data}");
            append_port_log(port_name, format!("CLI modbus {direction}: {data}"));
        }
        IpcMessage::Heartbeat { .. } => {
            // Heartbeat can be ignored for now or used for future monitoring
        }
        IpcMessage::StationsUpdate { stations_data, .. } => {
            log::info!(
                "CLI[{port_name}]: StationsUpdate received, {} bytes",
                stations_data.len()
            );

            // Deserialize and update the port's station configuration
            if let Ok(stations) =
                postcard::from_bytes::<Vec<crate::cli::config::StationConfig>>(&stations_data)
            {
                log::info!("CLI[{port_name}]: Decoded {} stations", stations.len());
                append_port_log(
                    port_name,
                    format!("CLI stations update: {} stations", stations.len()),
                );

                // TODO: Apply the stations update to the port's ModbusRegisterItem list
                // This will require converting from StationConfig format to ModbusRegisterItem format
                // For now, just log it
            } else {
                log::warn!("CLI[{port_name}]: Failed to deserialize stations data");
                append_port_log(
                    port_name,
                    "CLI stations update: failed to deserialize".to_string(),
                );
            }
        }
        IpcMessage::StateLockRequest { requester, .. } => {
            log::info!("CLI[{port_name}]: StateLockRequest from {requester}");
            append_port_log(
                port_name,
                format!("CLI state lock request from {requester}"),
            );
            // TODO: Implement state locking mechanism
        }
        IpcMessage::StateLockAck { locked, .. } => {
            log::info!("CLI[{port_name}]: StateLockAck locked={locked}");
            append_port_log(port_name, format!("CLI state lock ack: locked={locked}"));
            // TODO: Handle state lock acknowledgment
        }
        IpcMessage::Status {
            status, details, ..
        } => {
            let msg = if let Some(details) = details {
                format!("CLI status: {status} ({details})")
            } else {
                format!("CLI status: {status}")
            };
            log::info!("CLI[{port_name}]: {msg}");
            append_port_log(port_name, msg);
        }
        IpcMessage::Log { level, message, .. } => {
            log::info!("CLI[{port_name}]: log[{level}] {message}");
            append_port_log(port_name, format!("CLI log[{level}]: {message}"));
        }
    }
    Ok(())
}

pub fn start(matches: &clap::ArgMatches) -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");

    // Terminal is initialized inside the rendering thread to avoid sharing
    // a Terminal instance across threads. The rendering loop will create
    // and restore the terminal on its own.

    let app = Arc::new(RwLock::new(Status::default()));

    // Initialize the global status
    self::status::init_status(app.clone())?;

    // Check if debug CI E2E test mode is enabled
    let debug_ci_e2e_enabled = matches.get_flag("debug-ci-e2e-test");
    let debug_dump_shutdown = if debug_ci_e2e_enabled {
        log::info!("🔍 Debug CI E2E test mode enabled - starting status dump thread");
        crate::protocol::status::debug_dump::enable_debug_dump();

        let shutdown_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dump_path = std::path::PathBuf::from("/tmp/ci_tui_status.json");
        let shutdown_signal_clone = shutdown_signal.clone();

        crate::protocol::status::debug_dump::start_status_dump_thread(
            dump_path,
            Some(shutdown_signal_clone),
            || {
                crate::tui::status::TuiStatus::from_global_status().and_then(|status| {
                    serde_json::to_string_pretty(&status)
                        .map_err(|e| anyhow::anyhow!("Failed to serialize TUI status: {}", e))
                })
            },
        );

        Some(shutdown_signal)
    } else {
        None
    };

    // Load persisted port configurations
    if let Ok(persisted_configs) = persistence::load_port_configs() {
        if !persisted_configs.is_empty() {
            let count = persisted_configs.len();
            for (port_name, config) in persisted_configs {
                self::status::write_status(|status| {
                    if let Some(port_arc) = status.ports.map.get(&port_name) {
                        let mut port = port_arc.write();
                        port.config = config.clone();
                        log::info!("✅ Restored configuration for port: {}", port_name);
                    }
                    Ok(())
                })?;
            }
            log::info!("📂 Restored {} port configuration(s)", count);
        }
    }

    if std::env::var("AOBA_TUI_FORCE_ERROR").is_ok() {
        self::status::write_status(|g| {
            ui_error_set(
                g,
                Some((
                    "demo forced error: AOBA_TUI_FORCE_ERROR".to_string(),
                    chrono::Local::now(),
                )),
            );
            Ok(())
        })?;
    }

    // Create channels for three-thread architecture
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>(); // core -> ui
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>(); // ui -> core
    let bus = Bus::new(core_rx.clone(), ui_tx.clone());

    // Thread exit/reporting channel: threads send their Result<()> here when they exit
    let (thr_tx, thr_rx) = flume::unbounded::<Result<()>>();

    // Thread 1: Core processing thread - handles UiToCore and CoreToUi communication
    let core_handle = thread::spawn({
        let core_tx = core_tx.clone();
        let thr_tx = thr_tx.clone();
        let ui_rx = ui_rx.clone();

        move || thr_tx.send(run_core_thread(ui_rx, core_tx))
    });

    // Thread 2: Input handling thread - processes keyboard input
    let (input_kill_tx, input_kill_rx) = flume::bounded::<()>(1);
    let input_handle = thread::spawn({
        let bus = bus.clone();
        move || input::run_input_thread(bus, input_kill_rx)
    });

    // Thread 3: UI rendering loop - handles rendering based on Status
    // The rendering thread will initialize and restore the terminal itself.
    let render_handle = thread::spawn(move || run_rendering_loop(bus, thr_rx));

    // Rendering thread is responsible for terminal restoration; nothing to do here.

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

    // Stop debug dump thread if it was started
    if let Some(shutdown_signal) = debug_dump_shutdown {
        shutdown_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("🔍 Debug dump thread shutdown signal sent");
    }

    Ok(())
}

fn run_rendering_loop(bus: Bus, thr_rx: flume::Receiver<Result<()>>) -> Result<()> {
    // Initialize terminal inside rendering thread to avoid cross-thread Terminal usage
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    // Ensure terminal is restored on any early return
    let result = (|| {
        // Check whether any watched thread reported an error or exit
        loop {
            if let Ok(res) = thr_rx.try_recv() {
                if let Err(err) = res {
                    eprintln!("thread exited with error: {err:#}");
                    return Err(err);
                } else {
                    // thread exited successfully - treat as fatal and exit
                    log::info!("a monitored thread exited cleanly; shutting down");
                    return Ok(());
                }
            }
            // Wait for core signals with timeout
            let should_quit = match bus.core_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CoreToUi::Tick)
                | Ok(CoreToUi::Refreshed)
                | Ok(CoreToUi::Error)
                | Err(flume::RecvTimeoutError::Timeout) => {
                    // Redraw on refresh
                    false
                }
                _ => {
                    // Core thread died, exit
                    true
                }
            };

            if should_quit {
                break;
            }

            terminal.draw(|frame| {
                render_ui(frame).expect("Render failed");
            })?;
        }

        terminal.clear()?;
        Ok(())
    })();

    // Restore terminal state
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;

    // propagate inner result
    result
}

// Extracted core thread main function so it can return Result and use `?` for fallible ops.
fn run_core_thread(
    ui_rx: flume::Receiver<UiToCore>,
    core_tx: flume::Sender<CoreToUi>,
) -> Result<()> {
    let mut polling_enabled = true;
    let scan_interval = Duration::from_secs(30); // Reduced from 2s to 30s
    let mut last_scan = std::time::Instant::now() - scan_interval;
    let mut scan_in_progress = false; // Track if scan is currently running

    // do_scan extracted to module-level function below

    let mut last_modbus_run = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut subprocess_manager = SubprocessManager::new();
    loop {
        // Drain UI -> core messages
        let msg_count_before = ui_rx.len();
        let mut msg_count_processed = 0;
        while let Ok(msg) = ui_rx.try_recv() {
            msg_count_processed += 1;
            let msg_name = match &msg {
                UiToCore::Quit => "Quit".to_string(),
                UiToCore::Refresh => "Refresh".to_string(),
                UiToCore::PausePolling => "PausePolling".to_string(),
                UiToCore::ResumePolling => "ResumePolling".to_string(),
                UiToCore::ToggleRuntime(port) => format!("ToggleRuntime({port})"),
                UiToCore::SendRegisterUpdate {
                    port_name,
                    station_id,
                    start_address,
                    values,
                    ..
                } => {
                    format!(
                        "SendRegisterUpdate(port={port_name}, station={station_id}, addr={start_address}, values={values:?})"
                    )
                }
            };
            log::info!("🔵 Core thread received message: {msg_name}");
            match msg {
                UiToCore::Quit => {
                    log::info!("Received quit signal");
                    // Notify UI to quit and then exit core thread
                    core_tx
                        .send(CoreToUi::Quit)
                        .map_err(|err| anyhow!("Failed to send Quit to UI core: {err}"))?;
                    return Ok(());
                }
                UiToCore::Refresh => {
                    if crate::tui::utils::scan::scan_ports(&core_tx, &mut scan_in_progress)? {
                        last_scan = std::time::Instant::now();
                    }
                }
                UiToCore::PausePolling => {
                    polling_enabled = false;
                    if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                        log::warn!("ToggleRuntime: failed to send Refreshed: {err}");
                    }
                    // Log state after refresh
                    if let Err(err) = log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::ResumePolling => {
                    polling_enabled = true;
                    core_tx.send(CoreToUi::Refreshed).map_err(|err| {
                        anyhow!("Failed to send Refreshed event to UI core: {err}")
                    })?;
                    // Log state after refresh
                    if let Err(err) = log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::ToggleRuntime(port_name) => {
                    log::info!("ToggleRuntime requested for {port_name}");

                    let subprocess_info_opt = self::status::read_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            return Ok(port.subprocess_info.clone());
                        }
                        Ok(None)
                    })?;

                    if let Some(info) = subprocess_info_opt {
                        // TUI only manages CLI subprocesses, stop it
                        if let Err(err) = subprocess_manager.stop_subprocess(&port_name) {
                            log::warn!(
                                "ToggleRuntime: failed to stop CLI subprocess for {port_name}: {err}"
                            );
                        }

                        if let Some(path) = info.data_source_path.clone() {
                            if let Err(err) = fs::remove_file(&path) {
                                log::debug!(
                                    "ToggleRuntime: failed to remove data source {path}: {err}"
                                );
                            }
                        }

                        self::status::write_status(|status| {
                            if let Some(port) = status.ports.map.get_mut(&port_name) {
                                port.state = PortState::Free;
                                port.subprocess_info = None;
                                // Port is now stopped
                                port.status_indicator =
                                    types::port::PortStatusIndicator::NotStarted;
                            }
                            Ok(())
                        })?;

                        append_port_log(
                            &port_name,
                            "Stopped CLI subprocess managed by TUI".to_string(),
                        );

                        if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                            log::warn!("ToggleRuntime: failed to send Refreshed: {err}");
                        }
                        if let Err(err) = log_state_snapshot() {
                            log::warn!("Failed to log state snapshot: {err}");
                        }
                        continue;
                    }

                    // Extract CLI inputs WITHOUT holding any locks during subprocess operations
                    let cli_inputs = self::status::read_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            if let Some(result) = with_port_read(port, |port| {
                                let types::port::PortConfig::Modbus { mode, stations } =
                                    &port.config;
                                log::info!(
                                    "ToggleRuntime({port_name}): checking CLI inputs - mode={}, station_count={}",
                                    if mode.is_master() { "Master" } else { "Slave" },
                                    stations.len()
                                );
                                if !stations.is_empty() {
                                    let baud = port
                                        .state
                                        .runtime_handle()
                                        .map(|rt| rt.current_cfg.baud)
                                        .unwrap_or(9600);
                                    log::info!(
                                        "ToggleRuntime({port_name}): found {} station(s) - will attempt CLI subprocess",
                                        stations.len()
                                    );
                                    // For Master mode, pass all stations; for Slave, only first
                                    return Some((mode.clone(), stations.clone(), baud));
                                }
                                log::info!(
                                    "ToggleRuntime({port_name}): no station configured - will use native runtime"
                                );
                                None
                            }) {
                                return Ok(result);
                            }
                        }
                        Ok(None)
                    })?;
                    // Lock released here - safe to do long operations

                    let mut cli_started = false;

                    if let Some((mode, stations, baud_rate)) = cli_inputs {
                        match mode {
                            types::modbus::ModbusConnectionMode::Slave { .. } => {
                                // For Slave mode, use first station (slaves typically have one config)
                                let station = &stations[0];

                                log::info!(
                                    "ToggleRuntime: attempting to spawn CLI subprocess (SlavePoll) for {port_name}"
                                );

                                // Note: Slave mode polls external master, so no data source needed
                                let cli_config = CliSubprocessConfig {
                                    port_name: port_name.clone(),
                                    mode: CliMode::SlavePoll,
                                    station_id: station.station_id,
                                    register_address: station.register_address,
                                    register_length: station.register_length,
                                    register_mode: register_mode_to_cli_arg(station.register_mode)
                                        .to_string(),
                                    baud_rate,
                                    data_source: None,
                                };

                                // Spawn subprocess WITHOUT holding any status locks
                                match subprocess_manager.start_subprocess(cli_config) {
                                    Ok(()) => {
                                        if let Some(snapshot) =
                                            subprocess_manager.snapshot(&port_name)
                                        {
                                            log::info!(
                                                "ToggleRuntime: CLI subprocess spawned for {port_name} (mode={:?}, pid={:?})",
                                                snapshot.mode,
                                                snapshot.pid
                                            );
                                            let subprocess_info = PortSubprocessInfo {
                                                mode: cli_mode_to_port_mode(&snapshot.mode),
                                                ipc_socket_name: snapshot.ipc_socket_name.clone(),
                                                pid: snapshot.pid,
                                                data_source_path: None, // SlavePoll doesn't use data source
                                            };

                                            // Now update status with the result (short lock hold)
                                            self::status::write_status(|status| {
                                                if let Some(port) = status.ports.map.get_mut(&port_name) {
                                                    port.state = PortState::OccupiedByThis;
                                                    port.subprocess_info = Some(subprocess_info.clone());
                                                    // Port is now running
                                                    port.status_indicator = if port.config_modified {
                                                        types::port::PortStatusIndicator::RunningWithChanges
                                                    } else {
                                                        types::port::PortStatusIndicator::Running
                                                    };
                                                }
                                                Ok(())
                                            })?;

                                            append_port_log(
                                                &port_name,
                                                format!(
                                                    "Spawned CLI subprocess (mode: {:?}, pid: {:?})",
                                                    snapshot.mode, snapshot.pid
                                                ),
                                            );
                                            cli_started = true;
                                        } else {
                                            log::warn!(
                                                "ToggleRuntime: subprocess snapshot missing for {port_name}"
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        let msg = format!(
                                            "Failed to start CLI subprocess for {port_name}: {err}"
                                        );
                                        append_port_log(&port_name, msg.clone());
                                        self::status::write_status(|status| {
                                            status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                                                message: msg.clone(),
                                                timestamp: chrono::Local::now(),
                                            });
                                            Ok(())
                                        })?;
                                        // Note: No data source file to clean up for SlavePoll mode
                                    }
                                }
                            }
                            types::modbus::ModbusConnectionMode::Master => {
                                log::info!(
                                    "ToggleRuntime: attempting to spawn CLI subprocess (MasterProvide) for {port_name} with {} station(s)",
                                    stations.len()
                                );

                                // Initialize merged data source for all stations
                                let (
                                    data_source_path,
                                    merged_station_id,
                                    merged_start_addr,
                                    merged_length,
                                ) = initialize_cli_data_source(&port_name, &stations)?;

                                let cli_config = CliSubprocessConfig {
                                    port_name: port_name.clone(),
                                    mode: CliMode::MasterProvide,
                                    station_id: merged_station_id as u8,
                                    register_address: merged_start_addr,
                                    register_length: merged_length,
                                    register_mode: register_mode_to_cli_arg(
                                        stations[0].register_mode,
                                    )
                                    .to_string(),
                                    baud_rate,
                                    data_source: Some(format!(
                                        "file:{}",
                                        data_source_path.to_string_lossy()
                                    )),
                                };

                                // Spawn subprocess WITHOUT holding any status locks
                                match subprocess_manager.start_subprocess(cli_config) {
                                    Ok(()) => {
                                        if let Some(snapshot) =
                                            subprocess_manager.snapshot(&port_name)
                                        {
                                            log::info!(
                                                "ToggleRuntime: CLI subprocess spawned for {port_name} (mode={:?}, pid={:?}, data_source={})",
                                                snapshot.mode,
                                                snapshot.pid,
                                                data_source_path.display()
                                            );
                                            let subprocess_info = PortSubprocessInfo {
                                                mode: cli_mode_to_port_mode(&snapshot.mode),
                                                ipc_socket_name: snapshot.ipc_socket_name.clone(),
                                                pid: snapshot.pid,
                                                data_source_path: Some(
                                                    data_source_path
                                                        .to_string_lossy()
                                                        .to_string(),
                                                ),
                                            };

                                            // Now update status with the result (short lock hold)
                                            self::status::write_status(|status| {
                                                if let Some(port) = status.ports.map.get_mut(&port_name) {
                                                    port.state = PortState::OccupiedByThis;
                                                    port.subprocess_info = Some(subprocess_info.clone());
                                                    // Port is now running
                                                    port.status_indicator = if port.config_modified {
                                                        types::port::PortStatusIndicator::RunningWithChanges
                                                    } else {
                                                        types::port::PortStatusIndicator::Running
                                                    };
                                                }
                                                Ok(())
                                            })?;

                                            append_port_log(
                                                &port_name,
                                                format!(
                                                    "Spawned CLI subprocess (mode: {:?}, pid: {:?})",
                                                    snapshot.mode, snapshot.pid
                                                ),
                                            );
                                            cli_started = true;
                                        } else {
                                            log::warn!(
                                                "ToggleRuntime: subprocess snapshot missing for {port_name}"
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        let msg = format!(
                                            "Failed to start CLI subprocess for {port_name}: {err}"
                                        );
                                        append_port_log(&port_name, msg.clone());
                                        
                                        // Update port status indicator to show failure
                                        self::status::write_status(|status| {
                                            if let Some(port) = status.ports.map.get(&port_name) {
                                                if with_port_write(port, |port| {
                                                    port.status_indicator = types::port::PortStatusIndicator::StartupFailed {
                                                        error_message: err.to_string(),
                                                        timestamp: chrono::Local::now(),
                                                    };
                                                })
                                                .is_none()
                                                {
                                                    log::warn!(
                                                        "ToggleRuntime: failed to acquire write lock for {port_name} when setting failure status"
                                                    );
                                                }
                                            }
                                            
                                            status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                                                message: msg.clone(),
                                                timestamp: chrono::Local::now(),
                                            });
                                            Ok(())
                                        })?;

                                        if let Err(remove_err) = fs::remove_file(&data_source_path)
                                        {
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

                    // TUI no longer falls back to native runtime.
                    // If CLI subprocess fails to start, the port remains Free.
                    if !cli_started {
                        log::warn!(
                            "ToggleRuntime: CLI subprocess failed to start for {port_name}, port remains Free"
                        );
                    }

                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                    if let Err(err) = log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
                UiToCore::SendRegisterUpdate {
                    port_name,
                    station_id,
                    register_type,
                    start_address,
                    values,
                } => {
                    log::info!(
                        "🔵 SendRegisterUpdate requested for {port_name}: station={station_id}, type={register_type}, addr={start_address}, values={values:?}"
                    );

                    // Send individual register update (deprecated but kept for backward compatibility)
                    if let Err(err) = subprocess_manager.send_register_update(
                        &port_name,
                        station_id,
                        register_type,
                        start_address,
                        values,
                    ) {
                        log::warn!("❌ Failed to send register update to CLI subprocess for {port_name}: {err}");
                    } else {
                        log::info!(
                            "✅ Sent individual register update to CLI subprocess for {port_name}"
                        );
                    }

                    // Send full stations update for complete synchronization
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
            log::info!(
                "📊 Core thread: queue had {msg_count_before} messages, processed {msg_count_processed}",
            );
        }

        let dead_processes = subprocess_manager.reap_dead_processes();
        if !dead_processes.is_empty() {
            let mut cleanup_paths: HashMap<String, Option<String>> = HashMap::new();
            self::status::write_status(|status| {
                for (port_name, _) in &dead_processes {
                    if let Some(port) = status.ports.map.get_mut(port_name) {
                        if port.state.is_occupied_by_this() {
                            if let Some(info) = &port.subprocess_info {
                                cleanup_paths
                                    .insert(port_name.clone(), info.data_source_path.clone());
                            }
                            port.state = PortState::Free;
                            port.subprocess_info = None;
                            // Port is now stopped
                            port.status_indicator =
                                types::port::PortStatusIndicator::NotStarted;
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

                append_port_log(
                    &port_name,
                    format!("CLI subprocess exited: {exit_status:?}"),
                );

                if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                    log::warn!("Failed to send Refreshed after CLI exit for {port_name}: {err}");
                }
            }
        }

        for (port_name, message) in subprocess_manager.poll_ipc_messages() {
            if let Err(err) = handle_cli_ipc_message(&port_name, message) {
                log::warn!("Failed to handle IPC message for {port_name}: {err}");
            }
        }

        if polling_enabled
            && last_scan.elapsed() >= scan_interval
            && crate::tui::utils::scan::scan_ports(&core_tx, &mut scan_in_progress)?
        {
            last_scan = std::time::Instant::now();
        }

        // Update spinner frame for busy indicator animation
        self::status::write_status(|status| {
            status.temporarily.busy.spinner_frame =
                status.temporarily.busy.spinner_frame.wrapping_add(1);
            Ok(())
        })?;

        // Handle modbus communication at most once per 10ms to ensure very responsive behavior
        // This high frequency is critical for TUI Master mode to respond quickly to CLI slave requests
        // NOTE: The daemon handles Runtime-owned ports (legacy). TUI-spawned CLI subprocesses
        // communicate via IPC instead, but the daemon is still needed for backward compatibility.
        if polling_enabled && last_modbus_run.elapsed() >= std::time::Duration::from_millis(10) {
            // Update the timestamp first to ensure we don't re-enter while still running
            last_modbus_run = std::time::Instant::now();
            crate::protocol::daemon::handle_modbus_communication()?;
        }

        core_tx
            .send(CoreToUi::Tick)
            .map_err(|err| anyhow!("failed to send Tick: {err}"))?;
        thread::sleep(Duration::from_millis(50));
    }
}

/// Render UI function that only reads from Status (immutable reference)
fn render_ui(frame: &mut Frame) -> Result<()> {
    let area = frame.area();

    let bottom_height = self::status::read_status(|status| {
        let err_lines = if status.temporarily.error.is_some() {
            1
        } else {
            0
        };
        let hints_count = match crate::tui::ui::pages::bottom_hints_for_app() {
            Ok(h) => h.len(),
            Err(_) => 0,
        };
        Ok(hints_count + err_lines)
    })?;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(3),    // main
            Constraint::Length(bottom_height as u16),
        ])
        .split(area);

    // Use the new pages module for rendering
    crate::tui::ui::title::render_title(frame, main_chunks[0])?;
    crate::tui::ui::pages::render_panels(frame, main_chunks[1])?;
    crate::tui::ui::bottom::render_bottom(frame, main_chunks[2])?;

    Ok(())
}

/// Log a snapshot of the current TUI state for testing purposes.
/// This function is called after each Refreshed event to allow log-based
/// verification of state transitions.
pub fn log_state_snapshot() -> Result<()> {
    use crate::protocol::status::types::port::PortState;
    use serde_json::json;

    self::status::read_status(|status| {
        // Extract page info
        let page_name = match &status.page {
            crate::tui::status::Page::Entry { .. } => "Entry",
            crate::tui::status::Page::ConfigPanel { .. } => "ConfigPanel",
            crate::tui::status::Page::ModbusDashboard { .. } => "ModbusDashboard",
            crate::tui::status::Page::LogPanel { .. } => "LogPanel",
            crate::tui::status::Page::About { .. } => "About",
        };

        let cursor_info = match &status.page {
            crate::tui::status::Page::Entry { cursor, .. } => {
                if let Some(c) = cursor {
                    format!("{c:?}")
                } else {
                    "None".to_string()
                }
            }
            crate::tui::status::Page::ConfigPanel { cursor, .. } => {
                format!("{cursor:?}")
            }
            crate::tui::status::Page::ModbusDashboard { cursor, .. } => {
                format!("{cursor:?}")
            }
            _ => "N/A".to_string(),
        };

        // Extract port states
        let mut port_states = vec![];
        for port_name in &status.ports.order {
            if let Some(port_arc) = status.ports.map.get(port_name) {
                let port = port_arc.read();
                let state_str = match &port.state {
                    PortState::Free => "Free",
                    PortState::OccupiedByThis { owner: _ } => "OccupiedByThis",
                    PortState::OccupiedByOther => "OccupiedByOther",
                };
                port_states.push(json!({
                    "name": port_name,
                    "state": state_str,
                    "type": &port.port_type,
                }));
            }
        }

        // Extract config edit state
        let config_edit = json!({
            "active": status.temporarily.config_edit.active,
            "port": status.temporarily.config_edit.port,
            "field_index": status.temporarily.config_edit.field_index,
            "field_key": status.temporarily.config_edit.field_key,
            "buffer": status.temporarily.config_edit.buffer,
            "cursor_pos": status.temporarily.config_edit.cursor_pos,
        });

        // Build complete state snapshot
        let snapshot = json!({
            "page": page_name,
            "cursor": cursor_info,
            "ports": port_states,
            "config_edit": config_edit,
            "error": status.temporarily.error.as_ref().map(|e| &e.message),
        });

        // Log with STATE_DUMP prefix for easy parsing in tests
        log::info!("STATE_DUMP: {snapshot}");
        Ok(())
    })
}
