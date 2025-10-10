pub mod input;
pub mod subprocess;
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

    // Create subprocess manager
    let mut subprocess_manager = subprocess::SubprocessManager::new();

    // do_scan extracted to module-level function below

    let mut last_modbus_run = std::time::Instant::now() - std::time::Duration::from_secs(1);
    loop {
        // Poll IPC messages from subprocesses
        for (port_name, ipc_msg) = subprocess_manager.poll_ipc_messages() {
            match ipc_msg {
                crate::protocol::ipc::IpcMessage::PortOpened { port_name: _  } => {
                    log::info!("Subprocess for {} reported port opened", port_name);
                    // Update status to show port as occupied
                    write_status(|status| {
                        if let Some(port) = status.ports.map.get(&port_name) {
                            if with_port_write(port, |port| {
                                port.state = PortState::OccupiedByThis {
                                    handle: None,
                                    runtime: crate::protocol::runtime::PortRuntimeHandle {
                                        shared_serial: std::sync::Arc::new(std::sync::Mutex::new(None)),
                                        cmd_tx: flume::unbounded().0,
                                        evt_rx: flume::unbounded().1,
                                    },
                                };
                            })
                            .is_some()
                            {
                                // updated
                            }
                        }
                        Ok(())
                    })?;
                }
                crate::protocol::ipc::IpcMessage::PortError { port_name: _, error } => {
                    log::error!("Subprocess for {} reported error: {}", port_name, error);
                    // Set error in status
                    write_status(|status| {
                        status.temporarily.error = Some(types::ErrorInfo {
                            message: format!("Port {}: {}", port_name, error),
                            timestamp: chrono::Local::now(),
                        });
                        Ok(())
                    })?;
                }
                crate::protocol::ipc::IpcMessage::Shutdown => {
                    log::info!("Subprocess for {} is shutting down", port_name);
                }
                _ => {}
            }
        }
        
        // Check for dead subprocesses
        for (port_name, exit_status) in subprocess_manager.reap_dead_processes() {
            log::warn!("Subprocess for {} died with status: {:?}", port_name, exit_status);
            // Update status to show port as free
            write_status(|status| {
                if let Some(port) = status.ports.map.get(&port_name) {
                    if with_port_write(port, |port| {
                        port.state = PortState::Free;
                    })
                    .is_some()
                    {
                        // updated
                    }
                }
                Ok(())
            })?;
        }

        // Drain UI -> core messages
        while let Ok(msg) = ui_rx.try_recv() {
            match msg {
                UiToCore::Quit => {
                    log::info!("Received quit signal");
                    // Stop all subprocesses
                    subprocess_manager.shutdown_all();
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
                }
                UiToCore::ResumePolling => {
                    polling_enabled = true;
                    core_tx.send(CoreToUi::Refreshed).map_err(|err| {
                        anyhow!("Failed to send Refreshed event to UI core: {err}")
                    })?;
                }
                UiToCore::ToggleRuntime(port_name) => {
                    log::info!("ToggleRuntime requested for {port_name}");
                    
                    // Check if subprocess already exists for this port
                    let active_ports = subprocess_manager.active_ports();
                    if active_ports.contains(&port_name) {
                        // Stop the subprocess
                        log::info!("Stopping subprocess for {}", port_name);
                        if let Err(err) = subprocess_manager.stop_subprocess(&port_name) {
                            log::error!("Failed to stop subprocess for {}: {}", port_name, err);
                        }
                        
                        // Update status
                        write_status(|status| {
                            if let Some(port) = status.ports.map.get(&port_name) {
                                if with_port_write(port, |port| {
                                    port.state = PortState::Free;
                                })
                                .is_some()
                                {
                                    // updated
                                }
                            }
                            Ok(())
                        })?;
                    } else {
                        // Start a new subprocess
                        log::info!("Starting subprocess for {}", port_name);
                        
                        // Get modbus configuration from status
                        let modbus_config = read_status(|status| {
                            Ok(status.modbus.clone())
                        }).unwrap_or_default();
                        
                        // Determine mode based on modbus config
                        let mode = if modbus_config.is_master {
                            subprocess::CliMode::MasterProvide
                        } else {
                            subprocess::CliMode::SlaveListen
                        };
                        
                        // For now, use a dummy data source for master mode
                        // In a real implementation, this would come from the UI configuration
                        let data_source = if mode == subprocess::CliMode::MasterProvide {
                            // Create a temporary file with dummy data
                            let temp_file = std::env::temp_dir().join(format!("aoba-data-{}.jsonl", uuid::Uuid::new_v4()));
                            std::fs::write(&temp_file, "{\"values\":[1,2,3,4,5]}\n")?;
                            Some(format!("file:{}", temp_file.display()))
                        } else {
                            None
                        };
                        
                        let config = subprocess::CliSubprocessConfig {
                            port_name: port_name.clone(),
                            mode,
                            station_id: modbus_config.station_id,
                            register_address: modbus_config.register_address,
                            register_length: modbus_config.register_length,
                            register_mode: format!("{:?}", modbus_config.register_mode).to_lowercase(),
                            baud_rate: 9600, // TODO: Get from config
                            data_source,
                        };
                        
                        match subprocess_manager.start_subprocess(config) {
                            Ok(()) => {
                                log::info!("Successfully started subprocess for {}", port_name);
                            }
                            Err(err) => {
                                log::error!("Failed to start subprocess for {}: {}", port_name, err);
                                // Set error in status
                                write_status(|status| {
                                    status.temporarily.error = Some(types::ErrorInfo {
                                        message: format!("Failed to start subprocess for {}: {}", port_name, err),
                                        timestamp: chrono::Local::now(),
                                    });
                                    Ok(())
                                })?;
                            }
                        }
                    }

                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
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

        // Handle modbus communication at most once per 10ms to ensure very responsive behavior
        // This high frequency is critical for TUI Master mode to respond quickly to CLI slave requests
        // NOTE: With subprocess management, this might not be needed anymore as communication
        // happens in the CLI subprocesses. Keeping it for now for compatibility.
        if polling_enabled && last_modbus_run.elapsed() >= std::time::Duration::from_millis(10) {
            // Update the timestamp first to ensure we don't re-enter while still running
            last_modbus_run = std::time::Instant::now();
            // crate::protocol::daemon::handle_modbus_communication()?;
            // TODO: Remove this once fully transitioned to subprocess management
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
