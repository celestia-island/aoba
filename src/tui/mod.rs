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

    let mut last_modbus_run = std::time::Instant::now() - std::time::Duration::from_secs(1);
    loop {
        // Drain UI -> core messages
        while let Ok(msg) = ui_rx.try_recv() {
            match msg {
                UiToCore::Quit => {
                    log::info!("Received quit signal");
                    // Notify UI to quit and then exit core thread
                    core_tx
                        .send(CoreToUi::Quit)
                        .map_err(|e| anyhow!("Failed to send Quit to UI core: {}", e))?;
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
                        .map_err(|e| anyhow!("Failed to send Refreshed event to UI core: {}", e))?;
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
                            let warn_msg = format!("ToggleRuntime: failed to send Stop: {e}");
                            log::warn!("{warn_msg}");

                            // Also write to the port's logs so the UI shows the error
                            write_status(|status| {
                                if let Some(port) = status.ports.map.get(&port_name) {
                                    if with_port_write(port, |port| {
                                        port.logs.push(
                                            crate::protocol::status::types::port::PortLogEntry {
                                                when: chrono::Local::now(),
                                                raw: warn_msg.clone(),
                                                parsed: None,
                                            },
                                        );
                                        if port.logs.len() > 1000 {
                                            let excess = port.logs.len() - 1000;
                                            port.logs.drain(0..excess);
                                        }
                                        true
                                    })
                                    .is_none()
                                    {
                                        // If we couldn't acquire the port write lock, log a warning
                                        log::warn!("ToggleRuntime: failed to acquire write lock to append stop-warn for {port_name}");
                                    }
                                }
                                Ok(())
                            })?;
                        }
                        // Wait up to 1s for the runtime thread to emit Stopped. Use a single
                        // recv_timeout so we avoid short-interval polling while keeping the
                        // operation bounded. Treat failure non-fatally (log) to match
                        // surrounding code which tolerates stop failures.
                        match rt.evt_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                            Ok(evt) => {
                                if evt != crate::protocol::runtime::RuntimeEvent::Stopped {
                                    log::warn!(
                                                "ToggleRuntime: received unexpected event while stopping {port_name}: {evt:?}",
                                            );
                                }
                            }
                            Err(flume::RecvTimeoutError::Timeout) => {
                                log::warn!("ToggleRuntime: stop did not emit Stopped event within 1s for {port_name}");
                            }
                            Err(e) => {
                                log::warn!("ToggleRuntime: failed to receive Stopped event for {port_name}: {e}");
                            }
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
                                message: format!("Failed to start runtime: {e}"),
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

        // Handle modbus communication at most once per second to avoid tight loops
        if polling_enabled && last_modbus_run.elapsed() >= std::time::Duration::from_secs(1) {
            // Update the timestamp first to ensure we don't re-enter while still running
            last_modbus_run = std::time::Instant::now();
            crate::protocol::daemon::handle_modbus_communication()?;
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
