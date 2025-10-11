pub mod input;
pub mod subprocess;
pub mod ui;
pub mod utils;

use anyhow::{anyhow, Result};
use chrono::Local;
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use ratatui::{backend::CrosstermBackend, layout::*, prelude::*};

use crate::{
    protocol::{
        ipc::IpcMessage,
        status::{
            init_status, read_status,
            types::{
                self,
                port::{
                    PortLogEntry, PortOwner, PortState, PortSubprocessInfo, PortSubprocessMode,
                },
                Status,
            },
            with_port_read, with_port_write, write_status,
        },
    },
    tui::{
        subprocess::{CliMode, CliSubprocessConfig, SubprocessManager},
        ui::components::error_msg::ui_error_set,
        utils::bus::{Bus, CoreToUi, UiToCore},
    },
};

fn create_cli_data_source_path(port_name: &str) -> PathBuf {
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

    let timestamp = Local::now().format("%Y%m%d%H%M%S");
    let mut path = std::env::temp_dir();
    path.push(format!("aoba_cli_{}_{}.jsonl", fallback, timestamp));
    path
}

fn append_port_log(port_name: &str, raw: String) {
    let entry = PortLogEntry {
        when: Local::now(),
        raw,
        parsed: None,
    };

    if let Err(err) = write_status(|status| {
        if let Some(port) = status.ports.map.get(port_name) {
            if with_port_write(port, |port| {
                port.logs.push(entry.clone());
                if port.logs.len() > 1000 {
                    let excess = port.logs.len() - 1000;
                    port.logs.drain(0..excess);
                }
            })
            .is_none()
            {
                log::warn!(
                    "append_port_log: failed to acquire write lock for port {}",
                    port_name
                );
            }
        }
        Ok(())
    }) {
        log::warn!(
            "append_port_log: failed to persist log entry for {}: {}",
            port_name,
            err
        );
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
        CliMode::MasterProvide => PortSubprocessMode::MasterProvide,
    }
}

fn initialize_cli_data_source(
    port_name: &str,
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    register_address: u16,
    register_length: u16,
    register_mode: types::modbus::RegisterMode,
) -> Result<PathBuf> {
    let path = create_cli_data_source_path(port_name);
    if let Err(err) = write_cli_data_snapshot(
        &path,
        storage,
        register_address,
        register_length,
        register_mode,
        true,
    ) {
        log::error!(
            "initialize_cli_data_source: failed to write initial snapshot for {port_name}: {err}"
        );
        return Err(err);
    }
    log::info!(
        "initialize_cli_data_source: created data source for {port_name} at {}",
        path.display()
    );
    Ok(path)
}

fn write_cli_data_snapshot(
    path: &PathBuf,
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    register_address: u16,
    register_length: u16,
    register_mode: types::modbus::RegisterMode,
    truncate: bool,
) -> Result<()> {
    let values = crate::cli::modbus::extract_values_from_storage(
        storage,
        register_address,
        register_length,
        register_mode,
    )?;

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
    writeln!(file, "{}", serialized)?;
    Ok(())
}

pub(crate) fn append_cli_data_snapshot(
    path: &PathBuf,
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    register_address: u16,
    register_length: u16,
    register_mode: types::modbus::RegisterMode,
) -> Result<()> {
    write_cli_data_snapshot(
        path,
        storage,
        register_address,
        register_length,
        register_mode,
        false,
    )
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
            write_status(|status| {
                status.temporarily.error = Some(types::ErrorInfo {
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
        IpcMessage::RegisterUpdate { values, .. } => {
            log::info!("CLI[{port_name}]: RegisterUpdate {values:?}");
            append_port_log(port_name, format!("CLI register update: {:?}", values));
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

pub fn start() -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");

    // Terminal is initialized inside the rendering thread to avoid sharing
    // a Terminal instance across threads. The rendering loop will create
    // and restore the terminal on its own.

    let app = Arc::new(RwLock::new(Status::default()));

    // Initialize the global status
    init_status(app.clone())?;

    if std::env::var("AOBA_TUI_FORCE_ERROR").is_ok() {
        write_status(|g| {
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
        while let Ok(msg) = ui_rx.try_recv() {
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

                    let existing_owner = read_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            if let Some(owner) =
                                with_port_read(port, |port| port.state.owner().cloned())
                            {
                                return Ok(owner);
                            }
                        }
                        Ok(None)
                    })?;

                    if let Some(owner) = existing_owner {
                        match owner {
                            PortOwner::Runtime(rt) => {
                                write_status(|status| {
                                    if let Some(port) = status.ports.map.get(&port_name) {
                                        if with_port_write(port, |port| {
                                            port.state = PortState::Free;
                                        })
                                        .is_none()
                                        {
                                            log::warn!(
                                                "ToggleRuntime: failed to acquire write lock for {port_name} when clearing runtime"
                                            );
                                        }
                                    }
                                    Ok(())
                                })?;

                                if let Err(err) = rt
                                    .cmd_tx
                                    .send(crate::protocol::runtime::RuntimeCommand::Stop)
                                {
                                    let warn_msg =
                                        format!("ToggleRuntime: failed to send Stop: {err}");
                                    log::warn!("{warn_msg}");
                                    append_port_log(&port_name, warn_msg);
                                }

                                match rt.evt_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                                    Ok(evt) => {
                                        if evt != crate::protocol::runtime::RuntimeEvent::Stopped {
                                            log::warn!(
                                                "ToggleRuntime: received unexpected event while stopping {port_name}: {evt:?}"
                                            );
                                        }
                                    }
                                    Err(flume::RecvTimeoutError::Timeout) => {
                                        log::warn!("ToggleRuntime: stop did not emit Stopped event within 1s for {port_name}");
                                    }
                                    Err(err) => {
                                        log::warn!("ToggleRuntime: failed to receive Stopped event for {port_name}: {err}");
                                    }
                                }

                                if let Err(err) = core_tx.send(CoreToUi::Refreshed) {
                                    log::warn!("ToggleRuntime: failed to send Refreshed: {err}");
                                }
                                if let Err(err) = log_state_snapshot() {
                                    log::warn!("Failed to log state snapshot: {err}");
                                }
                                continue;
                            }
                            PortOwner::CliSubprocess(info) => {
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

                                write_status(|status| {
                                    if let Some(port) = status.ports.map.get(&port_name) {
                                        if with_port_write(port, |port| {
                                            port.state = PortState::Free;
                                        })
                                        .is_none()
                                        {
                                            log::warn!(
                                                "ToggleRuntime: failed to acquire write lock for {port_name} when clearing CLI owner"
                                            );
                                        }
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
                        }
                    }

                    let cli_inputs = read_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            if let Some(result) = with_port_read(port, |port| {
                                let types::port::PortConfig::Modbus { mode, stations } =
                                    &port.config;
                                if let Some(station) = stations.first() {
                                    let baud = port
                                        .state
                                        .runtime_handle()
                                        .map(|rt| rt.current_cfg.baud)
                                        .unwrap_or(9600);
                                    return Some((mode.clone(), station.clone(), baud));
                                }
                                None
                            }) {
                                return Ok(result);
                            }
                        }
                        Ok(None)
                    })?;

                    let mut cli_started = false;

                    if let Some((mode, station, baud_rate)) = cli_inputs {
                        match mode {
                            types::modbus::ModbusConnectionMode::Slave { storage, .. } => {
                                log::info!(
                                    "ToggleRuntime: attempting to spawn CLI subprocess (MasterProvide) for {port_name}"
                                );

                                let data_source_path = initialize_cli_data_source(
                                    &port_name,
                                    &storage,
                                    station.register_address,
                                    station.register_length,
                                    station.register_mode,
                                )?;

                                let cli_config = CliSubprocessConfig {
                                    port_name: port_name.clone(),
                                    mode: CliMode::MasterProvide,
                                    station_id: station.station_id,
                                    register_address: station.register_address,
                                    register_length: station.register_length,
                                    register_mode: register_mode_to_cli_arg(station.register_mode)
                                        .to_string(),
                                    baud_rate,
                                    data_source: Some(format!(
                                        "file:{}",
                                        data_source_path.to_string_lossy()
                                    )),
                                };

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
                                            let owner =
                                                PortOwner::CliSubprocess(PortSubprocessInfo {
                                                    mode: cli_mode_to_port_mode(&snapshot.mode),
                                                    ipc_socket_name: snapshot
                                                        .ipc_socket_name
                                                        .clone(),
                                                    pid: snapshot.pid,
                                                    data_source_path: Some(
                                                        data_source_path
                                                            .to_string_lossy()
                                                            .to_string(),
                                                    ),
                                                });

                                            write_status(|status| {
                                                if let Some(port) = status.ports.map.get(&port_name)
                                                {
                                                    if with_port_write(port, |port| {
                                                        port.state = PortState::OccupiedByThis {
                                                            owner: owner.clone(),
                                                        };
                                                    })
                                                    .is_none()
                                                    {
                                                        log::warn!(
                                                            "ToggleRuntime: failed to acquire write lock for {port_name} when marking CLI owner"
                                                        );
                                                    }
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
                                        write_status(|status| {
                                            status.temporarily.error = Some(types::ErrorInfo {
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
                            _ => {}
                        }
                    }

                    if !cli_started {
                        log::info!(
                            "ToggleRuntime: falling back to native runtime spawn for {port_name}"
                        );
                        let cfg = crate::protocol::runtime::SerialConfig::default();
                        let mut spawn_err: Option<anyhow::Error> = None;
                        let mut handle_opt: Option<crate::protocol::runtime::PortRuntimeHandle> =
                            None;
                        const MAX_RETRIES: usize = 8;
                        for attempt in 0..MAX_RETRIES {
                            match crate::protocol::runtime::PortRuntimeHandle::spawn(
                                port_name.clone(),
                                cfg.clone(),
                            ) {
                                Ok(h) => {
                                    handle_opt = Some(h);
                                    log::info!(
                                        "ToggleRuntime: Successfully spawned runtime for {port_name} on attempt {}",
                                        attempt + 1
                                    );
                                    break;
                                }
                                Err(err) => {
                                    spawn_err = Some(err);
                                    log::warn!(
                                        "ToggleRuntime: Failed to spawn runtime for {port_name} on attempt {}: {}",
                                        attempt + 1,
                                        spawn_err.as_ref().unwrap()
                                    );
                                    if attempt + 1 < MAX_RETRIES {
                                        let wait_ms = if attempt < 2 { 200 } else { 100 };
                                        std::thread::sleep(std::time::Duration::from_millis(
                                            wait_ms,
                                        ));
                                        continue;
                                    }
                                }
                            }
                        }

                        if let Some(handle) = handle_opt {
                            let handle_for_write = handle.clone();
                            write_status(|status| {
                                if let Some(port) = status.ports.map.get(&port_name) {
                                    if with_port_write(port, |port| {
                                        port.state = PortState::OccupiedByThis {
                                            owner: PortOwner::Runtime(handle_for_write.clone()),
                                        };
                                    })
                                    .is_none()
                                    {
                                        log::warn!(
                                            "ToggleRuntime: failed to acquire write lock for {port_name} when storing runtime handle"
                                        );
                                    }
                                }
                                Ok(())
                            })?;
                            append_port_log(
                                &port_name,
                                "Spawned native runtime for port".to_string(),
                            );
                        } else if let Some(err) = spawn_err {
                            write_status(|status| {
                                status.temporarily.error = Some(types::ErrorInfo {
                                    message: format!("Failed to start runtime: {err}"),
                                    timestamp: chrono::Local::now(),
                                });
                                Ok(())
                            })?;
                        }
                    }

                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                    if let Err(err) = log_state_snapshot() {
                        log::warn!("Failed to log state snapshot: {err}");
                    }
                }
            }
        }

        let dead_processes = subprocess_manager.reap_dead_processes();
        if !dead_processes.is_empty() {
            let mut cleanup_paths: HashMap<String, Option<String>> = HashMap::new();
            write_status(|status| {
                for (port_name, _) in &dead_processes {
                    if let Some(port) = status.ports.map.get(port_name) {
                        if with_port_write(port, |port| {
                            if let PortState::OccupiedByThis { owner } = &mut port.state {
                                if let PortOwner::CliSubprocess(info) = owner {
                                    cleanup_paths
                                        .insert(port_name.clone(), info.data_source_path.clone());
                                    port.state = PortState::Free;
                                }
                            }
                        })
                        .is_none()
                        {
                            log::warn!(
                                "Subprocess cleanup: failed to acquire write lock for {port_name}"
                            );
                        }
                    }
                }
                Ok(())
            })?;

            for (port_name, exit_status) in dead_processes {
                if let Some(path_opt) = cleanup_paths.remove(&port_name) {
                    if let Some(path) = path_opt {
                        if let Err(err) = fs::remove_file(&path) {
                            log::debug!("cleanup: failed to remove data source {path}: {err}");
                        }
                    }
                }

                append_port_log(
                    &port_name,
                    format!("CLI subprocess exited: {:?}", exit_status),
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
        write_status(|status| {
            status.temporarily.busy.spinner_frame =
                status.temporarily.busy.spinner_frame.wrapping_add(1);
            Ok(())
        })?;

        // Handle modbus communication at most once per 10ms to ensure very responsive behavior
        // This high frequency is critical for TUI Master mode to respond quickly to CLI slave requests
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

    let bottom_height = read_status(|status| {
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
    use crate::protocol::status::{read_status, types::port::PortState};
    use serde_json::json;

    read_status(|status| {
        // Extract page info
        let page_name = match &status.page {
            crate::protocol::status::types::Page::Entry { .. } => "Entry",
            crate::protocol::status::types::Page::ConfigPanel { .. } => "ConfigPanel",
            crate::protocol::status::types::Page::ModbusDashboard { .. } => "ModbusDashboard",
            crate::protocol::status::types::Page::LogPanel { .. } => "LogPanel",
            crate::protocol::status::types::Page::About { .. } => "About",
        };

        let cursor_info = match &status.page {
            crate::protocol::status::types::Page::Entry { cursor, .. } => {
                if let Some(c) = cursor {
                    format!("{c:?}")
                } else {
                    "None".to_string()
                }
            }
            crate::protocol::status::types::Page::ConfigPanel { cursor, .. } => {
                format!("{cursor:?}")
            }
            crate::protocol::status::types::Page::ModbusDashboard { cursor, .. } => {
                format!("{cursor:?}")
            }
            _ => "N/A".to_string(),
        };

        // Extract port states
        let mut port_states = vec![];
        for port_name in &status.ports.order {
            if let Some(port_arc) = status.ports.map.get(port_name) {
                if let Ok(port) = port_arc.read() {
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
