pub mod input;
pub mod ui;
pub mod utils;

use anyhow::Result;
use std::{
    io::{self, Stdout},
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use ratatui::{backend::CrosstermBackend, prelude::*};

use crate::{
    protocol::status::{
        types::{
            self,
            port::{PortData, PortLogEntry, PortState},
            Status,
        },
        StateManager, run_state_writer_thread,
    },
    tui::{
        ui::components::error_msg::ui_error_set,
        utils::bus::{Bus, CoreToUi, UiToCore},
    },
};

pub fn start() -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");

    // Setup terminal
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create StateManager and get the message receiver for the state writer thread
    let (state_manager, state_write_rx) = StateManager::new(Status::default());
    
    // Get the legacy Arc<RwLock<Status>> reference for compatibility
    let app = state_manager.get_state_ref().clone();

    if std::env::var("AOBA_TUI_FORCE_ERROR").is_ok() {
        let _ = state_manager.write_status_async(|g| {
            ui_error_set(
                g,
                Some((
                    "demo forced error: AOBA_TUI_FORCE_ERROR".to_string(),
                    chrono::Local::now(),
                )),
            );
            Ok(())
        });
    }

    // Create channels for thread architecture
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>(); // core -> ui
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>(); // ui -> core
    let bus = Bus::new(core_rx.clone(), ui_tx.clone());

    // Thread exit/reporting channel: threads send their Result<()> here when they exit
    let (thr_tx, thr_rx) = flume::unbounded::<Result<()>>();

    // Thread 0: State writer thread - processes all state write operations
    {
        let state_ref = app.clone();
        let thr_tx_writer = thr_tx.clone();
        thread::spawn(move || {
            let res = run_state_writer_thread(Status::default(), state_write_rx, state_ref);
            let _ = thr_tx_writer.send(res);
        });
    }

    // Thread 1: Core processing thread - handles UiToCore and CoreToUi communication
    {
        use crate::protocol::tty::available_ports_enriched;
        use chrono::Local;
        let state_mgr = state_manager.clone();
        let core_tx_clone = core_tx.clone();
        let thr_tx_clone_for_core = thr_tx.clone();
        thread::spawn(move || {
            let res = (|| -> Result<()> {
                let mut polling_enabled = true;
                let scan_interval = Duration::from_secs(30); // Reduced from 2s to 30s
                let mut last_scan = std::time::Instant::now() - scan_interval;
                let mut scan_in_progress = false; // Track if scan is currently running

                let do_scan = |state_mgr: &StateManager,
                               scan_in_progress: &mut bool|
                 -> bool {
                    // Return early if scan already in progress
                    if *scan_in_progress {
                        log::debug!("[CORE] Scan already in progress, skipping");
                        return false;
                    }

                    *scan_in_progress = true;

                    // Set busy indicator
                    let _ = state_mgr.write_status_async(|s| {
                        s.temporarily.busy.busy = true;
                        Ok(())
                    });

                    let ports = available_ports_enriched();
                    let scan_text = ports
                        .iter()
                        .map(|(info, extra)| format!("{} {:?}", info.port_name, extra))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let _ = state_mgr.write_status_async(move |s| {
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
                    });

                    *scan_in_progress = false;
                    // After adding ports to status, spawn per-port runtime listeners.
                    if let Ok(snapshot) = state_mgr.read_status(|s| Ok(s.clone()))
                    {
                        for port_name in snapshot.ports.order.iter() {
                            if let Some(pd) = snapshot.ports.map.get(port_name) {
                                if let Some(runtime) = pd.runtime.as_ref() {
                                    // Spawn a listener thread for this runtime's evt_rx.
                                    let evt_rx = runtime.evt_rx.clone();
                                    let state_mgr_clone = state_mgr.clone();
                                    let port_name_clone = port_name.clone();
                                    thread::spawn(move || {
                                        const MAX_LOGS: usize = 2000;
                                        while let Ok(evt) = evt_rx.recv() {
                                            match evt {
                                            crate::protocol::runtime::RuntimeEvent::FrameReceived(b)
                                            | crate::protocol::runtime::RuntimeEvent::FrameSent(b) => {
                                                let now = chrono::Local::now();
                                                let raw = b.iter().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join(" ");
                                                let parsed = Some(format!("{} bytes", b.len()));
                                                let entry = PortLogEntry {
                                                    when: now,
                                                    raw,
                                                    parsed,
                                                };
                                                let port_name_for_closure = port_name_clone.clone();
                                                let entry_for_closure = entry.clone();
                                                let _ = state_mgr_clone.write_status_async(move |s| {
                                                    if let Some(pdata) = s.ports.map.get_mut(&port_name_for_closure) {
                                                        pdata.logs.push(entry_for_closure);
                                                        if pdata.logs.len() > MAX_LOGS {
                                                            let drop = pdata.logs.len() - MAX_LOGS;
                                                            pdata.logs.drain(0..drop);
                                                        }
                                                        if pdata.log_auto_scroll {
                                                            pdata.log_selected = pdata.logs.len().saturating_sub(1);
                                                        }
                                                    }
                                                    Ok(())
                                                });
                                            }
                                            _ => {}
                                        }
                                        }
                                    });
                                }
                            }
                        }
                    }
                    let _ = core_tx_clone.send(CoreToUi::Refreshed);
                    true
                };

                loop {
                    // Drain UI -> core messages
                    while let Ok(msg) = ui_rx.try_recv() {
                        match msg {
                            UiToCore::Quit => {
                                log::info!("[CORE] Received quit signal");
                                // Notify UI to quit and then exit core thread
                                let _ = core_tx_clone.send(CoreToUi::Quit);
                                return Ok(());
                            }
                            UiToCore::Refresh => {
                                log::debug!("[CORE] immediate Refresh requested");
                                if do_scan(&state_mgr, &mut scan_in_progress) {
                                    last_scan = std::time::Instant::now();
                                } else {
                                    log::debug!("[CORE] immediate Refresh skipped because a scan is already running");
                                }
                            }
                            UiToCore::PausePolling => {
                                polling_enabled = false;
                                let _ = core_tx_clone.send(CoreToUi::Refreshed);
                            }
                            UiToCore::ResumePolling => {
                                polling_enabled = true;
                                let _ = core_tx_clone.send(CoreToUi::Refreshed);
                            }
                            UiToCore::ToggleRuntime(port_name) => {
                                log::info!("[CORE] ToggleRuntime requested for {port_name}");
                                use types::port::PortState;

                                // Step 1: extract any existing runtime handle (clone) so we can stop it
                                let existing_rt = state_mgr.read_status(|s| {
                                        if let Some(pd) = s.ports.map.get(&port_name) {
                                            Ok(pd.runtime.clone())
                                        } else {
                                            Ok(None)
                                        }
                                    })
                                    .unwrap_or(None);

                                if let Some(rt) = existing_rt {
                                    // Clear runtime reference in Status under write lock quickly
                                    let port_name_for_clear = port_name.clone();
                                    let _ = state_mgr.write_status_async(move |s| {
                                        if let Some(pd) = s.ports.map.get_mut(&port_name_for_clear) {
                                            pd.runtime = None;
                                            pd.state = PortState::Free;
                                        }
                                        Ok(())
                                    });

                                    // Send stop outside of the write lock and wait briefly for the runtime to acknowledge
                                    let _ = rt
                                        .cmd_tx
                                        .send(crate::protocol::runtime::RuntimeCommand::Stop);
                                    // Wait up to 1s for the runtime thread to emit Stopped, polling in 100ms intervals
                                    let mut stopped = false;
                                    for _ in 0..10 {
                                        match rt
                                        .evt_rx
                                        .recv_timeout(std::time::Duration::from_millis(100))
                                    {
                                        Ok(evt) => {
                                            if let crate::protocol::runtime::RuntimeEvent::Stopped =
                                                evt
                                            {
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
                                        log::warn!("[CORE] ToggleRuntime: stop did not emit Stopped event within timeout for {port_name}");
                                    }

                                    let _ = core_tx_clone.send(CoreToUi::Refreshed);
                                    continue;
                                }

                                // No runtime currently: attempt to spawn with a retry loop outside of any write lock
                                let cfg = crate::protocol::runtime::SerialConfig::default();
                                let mut spawn_err: Option<anyhow::Error> = None;
                                let mut handle_opt: Option<
                                    crate::protocol::runtime::PortRuntimeHandle,
                                > = None;
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
                                        Err(e) => {
                                            spawn_err = Some(e);
                                            // If not last attempt, wait a bit and retry; this allows the OS to release the port
                                            if attempt + 1 < MAX_RETRIES {
                                                // Slightly longer backoff on first attempts
                                                let wait_ms = if attempt < 2 { 200 } else { 100 };
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(wait_ms),
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                }

                                if let Some(handle) = handle_opt {
                                    // Clone handle for insertion under write lock to avoid moving
                                    let handle_for_write = handle.clone();
                                    let port_name_for_write = port_name.clone();
                                    // Write handle into status under write lock
                                    let _ = state_mgr.write_status_async(move |s| {
                                        if let Some(pd) = s.ports.map.get_mut(&port_name_for_write) {
                                            pd.runtime = Some(handle_for_write.clone());
                                            pd.state = PortState::OccupiedByThis;
                                        }
                                        Ok(())
                                    });
                                } else if let Some(e) = spawn_err {
                                    // All attempts failed: set transient error for UI
                                    let _ = state_mgr.write_status_async(move |s| {
                                        s.temporarily.error = Some(types::ErrorInfo {
                                            message: format!("failed to start runtime: {e}"),
                                            timestamp: chrono::Local::now(),
                                        });
                                        Ok(())
                                    });
                                }

                                let _ = core_tx_clone.send(CoreToUi::Refreshed);
                            }
                        }
                    }

                    if polling_enabled && last_scan.elapsed() >= scan_interval {
                        if do_scan(&state_mgr, &mut scan_in_progress) {
                            last_scan = std::time::Instant::now();
                        } else {
                            log::debug!(
                                "[CORE] scheduled scan skipped because a scan is already running"
                            );
                        }
                    }

                    // Update spinner frame for busy indicator animation
                    let _ = state_mgr.write_status_async(|s| {
                        s.temporarily.busy.spinner_frame =
                            s.temporarily.busy.spinner_frame.wrapping_add(1);
                        Ok(())
                    });

                    let _ = core_tx_clone.send(CoreToUi::Tick);
                    thread::sleep(Duration::from_millis(50));
                }
            })();
            let _ = thr_tx_clone_for_core.send(res);
        });
    }

    // Thread 2: Input handling thread - processes keyboard input
    input::spawn_input_thread(bus.clone(), Arc::clone(&app), thr_tx.clone());

    // Thread 3: UI rendering loop - handles rendering based on Status
    // Pass thr_rx so rendering loop can monitor other thread exits.
    let res = run_rendering_loop(&mut terminal, Arc::clone(&app), bus, thr_rx);

    // Restore terminal
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;

    res
}

fn run_rendering_loop(
    terminal: &mut Terminal<CrosstermBackend<&mut Stdout>>,
    app: Arc<RwLock<Status>>,
    bus: crate::tui::utils::bus::Bus,
    thr_rx: flume::Receiver<Result<()>>,
) -> Result<()> {
    loop {
        // Check whether any watched thread reported an error or exit
        if let Ok(res) = thr_rx.try_recv() {
            if let Err(e) = res {
                eprintln!("thread exited with error: {:#}", e);
                return Err(e);
            } else {
                // thread exited successfully - treat as fatal and exit
                log::info!("a monitored thread exited cleanly; shutting down");
                return Ok(());
            }
        }
        // Wait for core signals with timeout
        let should_quit = match bus.core_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(CoreToUi::Tick) => {
                // Redraw on tick
                false
            }
            Ok(CoreToUi::Refreshed) => {
                // Redraw on refresh
                false
            }
            Ok(CoreToUi::Error) => {
                // Redraw on error
                false
            }
            Ok(CoreToUi::Quit) => {
                // Core requested quit; terminate rendering loop
                true
            }
            Err(flume::RecvTimeoutError::Timeout) => {
                // Continue rendering loop on timeout
                false
            }
            Err(flume::RecvTimeoutError::Disconnected) => {
                // Core thread died, exit
                true
            }
        };

        if should_quit {
            break;
        }

        // Render UI - only read from Status. Use read_status to clone snapshot and
        // render from that to avoid holding the lock while rendering.
        if let Ok(snapshot) = crate::protocol::status::read_status(&app, |s| Ok(s.clone())) {
            terminal.draw(|f| {
                render_ui_readonly(f, &snapshot);
            })?;
        }
    }

    terminal.clear()?;
    Ok(())
}

/// Render UI function that only reads from Status (immutable reference)
fn render_ui_readonly(f: &mut Frame, app: &Status) {
    let area = f.area();
    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    let bottom_len = if app.temporarily.error.is_some() || subpage_active {
        2
    } else {
        1
    };
    let main_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Length(1), // title
            ratatui::layout::Constraint::Min(0),    // main
            ratatui::layout::Constraint::Length(bottom_len),
        ])
        .split(area);

    // Use the new pages module for rendering
    crate::tui::ui::title::render_title_readonly(f, main_chunks[0], app);
    crate::tui::ui::pages::render_panels(f, main_chunks[1], app);
    crate::tui::ui::bottom::render_bottom_readonly(f, main_chunks[2], app);
}
