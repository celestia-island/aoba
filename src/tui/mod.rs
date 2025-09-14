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
        write_status,
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
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = Arc::new(RwLock::new(Status::default()));

    // For manual testing: if AOBA_TUI_FORCE_ERROR is set, pre-populate an error to display
    if std::env::var("AOBA_TUI_FORCE_ERROR").is_ok() {
        let _ = write_status(&app, |g| {
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

    // Create channels for three-thread architecture
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>(); // core -> ui
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>(); // ui -> core
    let bus = Bus::new(core_rx.clone(), ui_tx.clone());

    // Thread 1: Core processing thread - handles UiToCore and CoreToUi communication
    {
        use crate::protocol::tty::available_ports_enriched;
        use chrono::Local;
        let _app_clone = Arc::clone(&app);
        let core_tx_clone = core_tx.clone();
        thread::spawn(move || {
            let mut polling_enabled = true;
            let scan_interval = Duration::from_secs(2);
            let mut last_scan = std::time::Instant::now() - scan_interval;

            let do_scan = |app_ref: &Arc<RwLock<Status>>| {
                let ports = available_ports_enriched();
                let scan_text = ports
                    .iter()
                    .map(|(info, extra)| format!("{} {:?}", info.port_name, extra))
                    .collect::<Vec<_>>()
                    .join("\n");

                let _ = write_status(app_ref, |s| {
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
                    Ok(())
                });
                // After adding ports to status, spawn per-port runtime listeners.
                if let Ok(snapshot) =
                    crate::protocol::status::read_status(app_ref, |s| Ok(s.clone()))
                {
                    for port_name in snapshot.ports.order.iter() {
                        if let Some(pd) = snapshot.ports.map.get(port_name) {
                            if let Some(runtime) = pd.runtime.as_ref() {
                                // Spawn a listener thread for this runtime's evt_rx.
                                let evt_rx = runtime.evt_rx.clone();
                                let app_clone3 = Arc::clone(app_ref);
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
                                                let _ = write_status(&app_clone3, |s| {
                                                    if let Some(pdata) = s.ports.map.get_mut(&port_name_clone) {
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
            };

            loop {
                // Drain UI -> core messages
                while let Ok(msg) = ui_rx.try_recv() {
                    match msg {
                        UiToCore::Quit => {
                            log::info!("[CORE] Received quit signal");
                            // Notify UI to quit and then exit core thread
                            let _ = core_tx_clone.send(CoreToUi::Quit);
                            return;
                        }
                        UiToCore::Refresh => {
                            log::debug!("[CORE] immediate Refresh requested");
                            do_scan(&_app_clone);
                            last_scan = std::time::Instant::now();
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
                            let existing_rt =
                                crate::protocol::status::read_status(&_app_clone, |s| {
                                    if let Some(pd) = s.ports.map.get(&port_name) {
                                        Ok(pd.runtime.clone())
                                    } else {
                                        Ok(None)
                                    }
                                })
                                .unwrap_or(None);

                            if let Some(rt) = existing_rt {
                                // Clear runtime reference in Status under write lock quickly
                                let _ = write_status(&_app_clone, |s| {
                                    if let Some(pd) = s.ports.map.get_mut(&port_name) {
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
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                wait_ms,
                                            ));
                                            continue;
                                        }
                                    }
                                }
                            }

                            if let Some(handle) = handle_opt {
                                // Clone handle for insertion under write lock to avoid moving
                                let handle_for_write = handle.clone();
                                // Write handle into status under write lock
                                let _ = write_status(&_app_clone, |s| {
                                    if let Some(pd) = s.ports.map.get_mut(&port_name) {
                                        pd.runtime = Some(handle_for_write.clone());
                                        pd.state = PortState::OccupiedByThis;
                                    }
                                    Ok(())
                                });
                            } else if let Some(e) = spawn_err {
                                // All attempts failed: set transient error for UI
                                let _ = write_status(&_app_clone, |s| {
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
                    do_scan(&_app_clone);
                    last_scan = std::time::Instant::now();
                }

                let _ = core_tx_clone.send(CoreToUi::Tick);
                thread::sleep(Duration::from_millis(50));
            }
        });
    }

    // Thread 2: Input handling thread - processes keyboard input
    input::spawn_input_thread(bus.clone(), Arc::clone(&app));

    // Thread 3: UI rendering thread - handles rendering based on Status
    let res = run_rendering_loop(&mut terminal, Arc::clone(&app), bus);

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
) -> Result<()> {
    loop {
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

    if app.temporarily.modals.mode_selector.active {
        let idx = app
            .temporarily
            .modals
            .mode_selector
            .selector
            .as_usize()
            .min(1);
        crate::tui::ui::components::mode_selector::render_mode_selector(f, idx);
    }
}
