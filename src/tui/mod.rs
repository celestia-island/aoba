pub mod input;
pub mod ui;
pub mod utils;

use anyhow::{anyhow, Result};
use std::{
    io,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use ratatui::{backend::CrosstermBackend, layout::*, prelude::*};

use rmodbus::server::context::ModbusContext;
use crate::{
    protocol::status::{
        init_status, read_status,
        types::{self, port::PortState, Status},
        with_port_read, with_port_write, write_status,
    },
    tui::{
        ui::components::error_msg::ui_error_set,
        utils::bus::{Bus, CoreToUi, UiToCore},
    },
};

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
        .map_err(|err| anyhow!("Failed to join core thread: {:?}", err))??;
    render_handle
        .join()
        .map_err(|err| anyhow!("Failed to join render thread: {:?}", err))??;

    input_kill_tx.send(())?;
    input_handle
        .join()
        .map_err(|err| anyhow!("Failed to join input thread: {:?}", err))??;
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

    loop {
        // Drain UI -> core messages
        while let Ok(msg) = ui_rx.try_recv() {
            match msg {
                UiToCore::Quit => {
                    log::info!("Received quit signal");
                    // Notify UI to quit and then exit core thread
                    core_tx
                        .send(CoreToUi::Quit)
                        .map_err(|e| anyhow!("failed to send Quit: {}", e))?;
                    return Ok(());
                }
                UiToCore::Refresh => {
                    if crate::tui::utils::scan::scan_ports(&core_tx, &mut scan_in_progress)? {
                        last_scan = std::time::Instant::now();
                    }
                }
                UiToCore::PausePolling => {
                    polling_enabled = false;
                    if let Err(e) = core_tx.send(CoreToUi::Refreshed) {
                        log::warn!("ToggleRuntime: failed to send Refreshed: {e}");
                    }
                }
                UiToCore::ResumePolling => {
                    polling_enabled = true;
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|e| anyhow!("failed to send Refreshed: {}", e))?;
                }
                UiToCore::ToggleRuntime(port_name) => {
                    log::info!("ToggleRuntime requested for {port_name}");
                    // Step 1: extract any existing runtime handle (clone) so we can stop it
                    let existing_rt = read_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            // use helper to avoid panics on poisoned locks
                            if let Some(opt_rt) = with_port_read(port, |port| {
                                if let types::port::PortState::OccupiedByThis { runtime, .. } =
                                    &port.state
                                {
                                    Some(runtime.clone())
                                } else {
                                    None
                                }
                            }) {
                                Ok(opt_rt)
                            } else {
                                Ok(None)
                            }
                        } else {
                            Ok(None)
                        }
                    })
                    .unwrap_or(None);

                    if let Some(rt) = existing_rt {
                        // Clear runtime reference in Status under write lock quickly
                        write_status(|status| {
                            if let Some(port) = status.ports.map.get(&port_name) {
                                if with_port_write(port, |port| {
                                    port.state = PortState::Free;
                                })
                                .is_some()
                                {
                                    // updated
                                } else {
                                    log::warn!("ToggleRuntime: failed to acquire write lock for {port_name} (Free)");
                                }
                            }
                            Ok(())
                        })?;

                        // Send stop outside of the write lock and wait briefly for the runtime to acknowledge
                        // Try to request runtime stop. Sending may fail if the runtime
                        // already exited; treat that non-fatally to avoid bringing down
                        // the entire core thread when user double-presses Enter.
                        if let Err(e) = rt
                            .cmd_tx
                            .send(crate::protocol::runtime::RuntimeCommand::Stop)
                        {
                            log::warn!("ToggleRuntime: failed to send Stop: {e}");
                        }
                        // Wait up to 1s for the runtime thread to emit Stopped, polling in 100ms intervals
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
                                    // timed out waiting this interval - continue waiting
                                }
                            }
                        }
                        if !stopped {
                            log::warn!("ToggleRuntime: stop did not emit Stopped event within timeout for {port_name}");
                        }

                        if let Err(e) = core_tx.send(CoreToUi::Refreshed) {
                            log::warn!("ToggleRuntime: failed to send Refreshed: {e}");
                        }
                        continue;
                    }

                    // No runtime currently: attempt to spawn with a retry loop outside of any write lock
                    let cfg = crate::protocol::runtime::SerialConfig::default();
                    let mut spawn_err: Option<anyhow::Error> = None;
                    let mut handle_opt: Option<crate::protocol::runtime::PortRuntimeHandle> = None;
                    const MAX_RETRIES: usize = 8;
                    for attempt in 0..MAX_RETRIES {
                        match crate::protocol::runtime::PortRuntimeHandle::spawn(
                            port_name.clone(),
                            cfg.clone(),
                        ) {
                            Ok(h) => {
                                handle_opt = Some(h);
                                break;
                            }
                            Err(err) => {
                                spawn_err = Some(err);
                                // If not last attempt, wait a bit and retry; this allows the OS to release the port
                                if attempt + 1 < MAX_RETRIES {
                                    // Slightly longer backoff on first attempts
                                    let wait_ms = if attempt < 2 { 200 } else { 100 };
                                    std::thread::sleep(std::time::Duration::from_millis(wait_ms));
                                    continue;
                                }
                            }
                        }
                    }

                    if let Some(handle) = handle_opt {
                        // Clone handle for insertion under write lock to avoid moving
                        let handle_for_write = handle.clone();
                        // Write handle into status under write lock
                        write_status(|status| {
                            if let Some(port) = status.ports.map.get(&port_name) {
                                if with_port_write(port, |port| {
                                    port.state = PortState::OccupiedByThis {
                                        handle: Some(types::port::SerialPortWrapper::new(
                                            handle_for_write.shared_serial.clone(),
                                        )),
                                        runtime: handle_for_write.clone(),
                                    };
                                })
                                .is_some()
                                {
                                    // updated
                                } else {
                                    log::warn!("ToggleRuntime: failed to acquire write lock for {port_name} (OccupiedByThis)");
                                }
                            }
                            Ok(())
                        })?;
                    } else if let Some(e) = spawn_err {
                        // All attempts failed: set transient error for UI
                        write_status(|status| {
                            status.temporarily.error = Some(types::ErrorInfo {
                                message: format!("failed to start runtime: {e}"),
                                timestamp: chrono::Local::now(),
                            });
                            Ok(())
                        })?;
                    }

                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|e| anyhow!("failed to send Refreshed: {}", e))?;
                }
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

        // Handle modbus communication
        if polling_enabled {
            handle_modbus_communication(&core_tx)?;
        }

        core_tx
            .send(CoreToUi::Tick)
            .map_err(|e| anyhow!("failed to send Tick: {}", e))?;
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

/// Handle modbus communication for all active ports
fn handle_modbus_communication(_core_tx: &flume::Sender<CoreToUi>) -> Result<()> {
    let now = std::time::Instant::now();
    
    // Get all ports that are currently active
    let active_ports = read_status(|status| {
        let mut ports = Vec::new();
        for (port_name, port_arc) in &status.ports.map {
            if let Ok(port_data) = port_arc.read() {
                if let types::port::PortState::OccupiedByThis { runtime: _, .. } = &port_data.state {
                    let types::port::PortConfig::Modbus { mode, stations } = &port_data.config;
                    if !stations.is_empty() {
                        ports.push((port_name.clone(), port_arc.clone(), *mode, stations.clone()));
                    }
                }
            }
        }
        Ok(ports)
    })?;

    for (port_name, port_arc, global_mode, stations) in active_ports {
        // Process each port's modbus communication
        match global_mode {
            types::modbus::ModbusConnectionMode::Master => {
                handle_master_polling(&port_name, &port_arc, &stations, now)?;
            }
            types::modbus::ModbusConnectionMode::Slave => {
                handle_slave_responses(&port_name, &port_arc, &stations)?;
            }
        }
    }

    Ok(())
}

/// Handle master polling for a specific port
fn handle_master_polling(
    port_name: &str,
    port_arc: &std::sync::Arc<std::sync::RwLock<types::port::PortData>>,
    stations: &[types::modbus::ModbusRegisterItem],
    now: std::time::Instant,
) -> Result<()> {
    // Get runtime handle for sending requests
    let runtime_handle = with_port_read(port_arc, |port| {
        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
            Some(runtime.clone())
        } else {
            None
        }
    });

    let Some(Some(runtime)) = runtime_handle else {
        return Ok(());
    };

    // Process each station for master polling
    for (index, station) in stations.iter().enumerate() {
        if now >= station.next_poll_at && station.pending_requests.is_empty() {
            // Time to send a new request
            let request_result = generate_modbus_request(station);
            
            match request_result {
                Ok(request_bytes) => {
                    // Send the request
                    if let Err(e) = runtime.cmd_tx.send(crate::protocol::runtime::RuntimeCommand::Write(request_bytes.clone())) {
                        log::warn!("Failed to send modbus request for {port_name} station {}: {e}", station.station_id);
                        continue;
                    }

                    // Log the sent frame
                    let hex_frame = request_bytes
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    
                    let log_entry = types::port::PortLogEntry {
                        when: chrono::Local::now(),
                        raw: format!("Master TX: {}", hex_frame),
                        parsed: None,
                    };

                    // Add log entry to port logs
                    with_port_write(port_arc, |port| {
                        port.logs.push(log_entry);
                        // Keep only the last 1000 log entries
                        if port.logs.len() > 1000 {
                            let excess = port.logs.len() - 1000;
                            port.logs.drain(0..excess);
                        }
                    });

                    // Update station polling state
                    with_port_write(port_arc, |port| {
                        let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
                        if let Some(station) = stations.get_mut(index) {
                            station.req_total = station.req_total.saturating_add(1);
                            station.next_poll_at = now + std::time::Duration::from_millis(2000); // 2 second interval
                        }
                    });

                    log::info!("Sent modbus master request for {port_name} station {}: {hex_frame}", station.station_id);
                }
                Err(e) => {
                    log::warn!("Failed to generate modbus request for {port_name} station {}: {e}", station.station_id);
                }
            }
        }
    }

    // Process incoming responses
    while let Ok(event) = runtime.evt_rx.try_recv() {
        match event {
            crate::protocol::runtime::RuntimeEvent::FrameReceived(frame) => {
                let hex_frame = frame
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");

                let log_entry = types::port::PortLogEntry {
                    when: chrono::Local::now(),
                    raw: format!("Master RX: {}", hex_frame),
                    parsed: None,
                };

                // Add log entry to port logs
                with_port_write(port_arc, |port| {
                    port.logs.push(log_entry);
                    // Keep only the last 1000 log entries
                    if port.logs.len() > 1000 {
                        let excess = port.logs.len() - 1000;
                        port.logs.drain(0..excess);
                    }
                });

                log::info!("Received modbus master response for {port_name}: {hex_frame}");
            }
            crate::protocol::runtime::RuntimeEvent::Error(error) => {
                log::warn!("Modbus runtime error for {port_name}: {error}");
            }
            _ => {}
        }
    }

    Ok(())
}

/// Handle slave responses for a specific port
fn handle_slave_responses(
    port_name: &str,
    port_arc: &std::sync::Arc<std::sync::RwLock<types::port::PortData>>,
    stations: &[types::modbus::ModbusRegisterItem],
) -> Result<()> {
    // Get runtime handle for receiving requests and sending responses
    let runtime_handle = with_port_read(port_arc, |port| {
        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
            Some(runtime.clone())
        } else {
            None
        }
    });

    let Some(Some(runtime)) = runtime_handle else {
        return Ok(());
    };

    // Process incoming requests and generate responses
    while let Ok(event) = runtime.evt_rx.try_recv() {
        match event {
            crate::protocol::runtime::RuntimeEvent::FrameReceived(frame) => {
                let hex_frame = frame
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");

                // Log the received request
                let log_entry = types::port::PortLogEntry {
                    when: chrono::Local::now(),
                    raw: format!("Slave RX: {}", hex_frame),
                    parsed: None,
                };

                with_port_write(port_arc, |port| {
                    port.logs.push(log_entry);
                    if port.logs.len() > 1000 {
                        let excess = port.logs.len() - 1000;
                        port.logs.drain(0..excess);
                    }
                });

                // Try to parse and respond to the request
                if let Ok(response) = generate_modbus_slave_response(&frame, stations) {
                    // Send the response
                    if let Err(e) = runtime.cmd_tx.send(crate::protocol::runtime::RuntimeCommand::Write(response.clone())) {
                        log::warn!("Failed to send modbus slave response for {port_name}: {e}");
                        continue;
                    }

                    // Log the sent response
                    let hex_response = response
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    
                    let log_entry = types::port::PortLogEntry {
                        when: chrono::Local::now(),
                        raw: format!("Slave TX: {}", hex_response),
                        parsed: None,
                    };

                    with_port_write(port_arc, |port| {
                        port.logs.push(log_entry);
                        if port.logs.len() > 1000 {
                            let excess = port.logs.len() - 1000;
                            port.logs.drain(0..excess);
                        }
                    });

                    log::info!("Sent modbus slave response for {port_name}: {hex_response}");
                } else {
                    log::debug!("Could not generate response for modbus request: {hex_frame}");
                }
            }
            crate::protocol::runtime::RuntimeEvent::Error(error) => {
                log::warn!("Modbus runtime error for {port_name}: {error}");
            }
            _ => {}
        }
    }

    Ok(())
}

/// Generate a modbus request for master polling
fn generate_modbus_request(station: &types::modbus::ModbusRegisterItem) -> Result<Vec<u8>> {
    use crate::protocol::modbus::*;

    let length = station.register_length.min(125); // Limit to max modbus length
    let address = station.register_address;
    let slave_id = station.station_id;

    match station.register_mode {
        types::modbus::RegisterMode::Coils => {
            let (_, raw) = generate_pull_get_coils_request(slave_id, address, length)?;
            Ok(raw)
        }
        types::modbus::RegisterMode::DiscreteInputs => {
            let (_, raw) = generate_pull_get_discrete_inputs_request(slave_id, address, length)?;
            Ok(raw)
        }
        types::modbus::RegisterMode::Holding => {
            let (_, raw) = generate_pull_get_holdings_request(slave_id, address, length)?;
            Ok(raw)
        }
        types::modbus::RegisterMode::Input => {
            let (_, raw) = generate_pull_get_inputs_request(slave_id, address, length)?;
            Ok(raw)
        }
    }
}

/// Generate a modbus slave response to an incoming request
fn generate_modbus_slave_response(
    request: &[u8],
    stations: &[types::modbus::ModbusRegisterItem],
) -> Result<Vec<u8>> {
    use rmodbus::server::{storage::ModbusStorageSmall, ModbusFrame};
    use rmodbus::ModbusProto;

    if request.len() < 2 {
        return Err(anyhow::anyhow!("Request too short"));
    }

    let slave_id = request[0];
    
    // Find a station configuration that matches the slave ID
    let _station = stations
        .iter()
        .find(|s| s.station_id == slave_id)
        .ok_or_else(|| anyhow::anyhow!("No station configured for slave ID {}", slave_id))?;

    // Create a simple storage context with some default values
    let mut context = ModbusStorageSmall::new();
    
    // Set some example values for demonstration
    for i in 0..100 {
        let _ = context.set_coil(i, i % 2 == 0);
        let _ = context.set_discrete(i, i % 3 == 0);
        let _ = context.set_holding(i, i as u16 * 10);
        let _ = context.set_input(i, i as u16 * 20);
    }

    let mut response = Vec::new();
    let mut frame = ModbusFrame::new(slave_id, request, ModbusProto::Rtu, &mut response);
    frame.parse()?;

    // Use the existing modbus helper functions to build responses
    match frame.func {
        rmodbus::consts::ModbusFunction::GetCoils => {
            if let Ok(Some(ret)) = crate::protocol::modbus::build_slave_coils_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow::anyhow!("Failed to build coils response"))
            }
        }
        rmodbus::consts::ModbusFunction::GetDiscretes => {
            if let Ok(Some(ret)) = crate::protocol::modbus::build_slave_discrete_inputs_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow::anyhow!("Failed to build discrete inputs response"))
            }
        }
        rmodbus::consts::ModbusFunction::GetHoldings => {
            if let Ok(Some(ret)) = crate::protocol::modbus::build_slave_holdings_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow::anyhow!("Failed to build holdings response"))
            }
        }
        rmodbus::consts::ModbusFunction::GetInputs => {
            if let Ok(Some(ret)) = crate::protocol::modbus::build_slave_inputs_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow::anyhow!("Failed to build inputs response"))
            }
        }
        _ => Err(anyhow::anyhow!("Unsupported modbus function code: {:?}", frame.func)),
    }
}
