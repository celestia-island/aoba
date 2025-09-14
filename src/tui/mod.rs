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
            ui::SpecialEntry,
            Status,
        },
        write_status,
    },
    tui::{
        ui::components::error_msg::ui_error_set,
        ui::pages::about,
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
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
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
    {
        let bus_clone = bus.clone();
        let app_clone = Arc::clone(&app);
        thread::spawn(move || {
            loop {
                // Poll for input
                if let Ok(true) = crossterm::event::poll(Duration::from_millis(100)) {
                    if let Ok(ev) = crossterm::event::read() {
                        // Support both Key and Mouse scroll events. Map Mouse ScrollUp/Down to
                        // synthesized KeyEvent Up/Down so existing key handlers can be reused.
                        let mut key_opt: Option<crossterm::event::KeyEvent> = None;
                        match ev {
                            crossterm::event::Event::Key(k) => key_opt = Some(k),
                            crossterm::event::Event::Mouse(me) => {
                                // First: if About page is active, let it consume the mouse scroll.
                                let mut consumed_by_page = false;
                                if let Ok(snapshot) =
                                    crate::protocol::status::read_status(&app_clone, |s| {
                                        Ok(s.clone())
                                    })
                                {
                                    // Check if About is active (either selected virtual entry or full page)
                                    let about_idx = snapshot.ports.order.len().saturating_add(2);
                                    let sel = match &snapshot.page {
                                        types::Page::Entry { cursor } => match cursor {
                                            Some(types::ui::EntryCursor::Com { idx }) => *idx,
                                            Some(types::ui::EntryCursor::Refresh) => {
                                                snapshot.ports.order.len()
                                            }
                                            Some(types::ui::EntryCursor::CreateVirtual) => {
                                                snapshot.ports.order.len().saturating_add(1)
                                            }
                                            Some(types::ui::EntryCursor::About) => {
                                                snapshot.ports.order.len().saturating_add(2)
                                            }
                                            None => 0usize,
                                        },
                                        types::Page::ModbusDashboard { selected_port, .. }
                                        | types::Page::ModbusConfig { selected_port, .. }
                                        | types::Page::ModbusLog { selected_port, .. } => {
                                            *selected_port
                                        }
                                        types::Page::About { .. } => about_idx,
                                    };

                                    if sel == about_idx {
                                        // Snapshot for about page input
                                        let snap_about = snapshot.snapshot_about();
                                        consumed_by_page = about::handle_mouse(
                                            me,
                                            &snapshot,
                                            &bus_clone,
                                            &app_clone,
                                            &snap_about,
                                        );
                                    }
                                }

                                if consumed_by_page {
                                    // Page consumed the mouse event; do not map to a key
                                } else {
                                    // Fallback: map scroll to Up/Down key events for global handling
                                    use crossterm::event::MouseEventKind as MEK;
                                    match me.kind {
                                        MEK::ScrollUp => {
                                            key_opt = Some(crossterm::event::KeyEvent::new(
                                                crossterm::event::KeyCode::Up,
                                                crossterm::event::KeyModifiers::NONE,
                                            ));
                                        }
                                        MEK::ScrollDown => {
                                            key_opt = Some(crossterm::event::KeyEvent::new(
                                                crossterm::event::KeyCode::Down,
                                                crossterm::event::KeyModifiers::NONE,
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => {}
                        }

                        if let Some(key) = key_opt {
                            if key.kind != crossterm::event::KeyEventKind::Press {
                                continue; // Ignore non-initial key press (repeat / release)
                            }

                            // Handle global quit
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                            {
                                if let crossterm::event::KeyCode::Char('c') = key.code {
                                    let _ = bus_clone.ui_tx.send(UiToCore::Quit);
                                    break;
                                }
                            }

                            // Route input to appropriate page handler. Use read_status to read a
                            // cloned snapshot of Status instead of directly locking the RwLock.
                            if let Ok(snapshot) =
                                crate::protocol::status::read_status(&app_clone, |s| Ok(s.clone()))
                            {
                                // First, let subpages consume input if applicable
                                let consumed = crate::tui::ui::pages::handle_input_in_subpage(
                                    key, &snapshot, &bus_clone, &app_clone,
                                );

                                if !consumed {
                                    // Handle global keys first
                                    use crate::tui::input::map_key;
                                    match map_key(key.code) {
                                        crate::tui::input::Action::Quit => {
                                            let _ = bus_clone.ui_tx.send(UiToCore::Quit);
                                            break;
                                        }
                                        crate::tui::input::Action::MoveNext => {
                                            // Move selection down
                                            let _ = write_status(&app_clone, |s| {
                                                let special_base = s.ports.order.len();
                                                let extra_count = SpecialEntry::all().len();
                                                let total = special_base + extra_count; // dynamic extras length
                                                                                        // Read current selection from page
                                                let mut sel = match &s.page {
                                                    types::Page::Entry { cursor } => match cursor {
                                                        Some(types::ui::EntryCursor::Com {
                                                            idx,
                                                        }) => *idx,
                                                        Some(types::ui::EntryCursor::Refresh) => {
                                                            s.ports.order.len()
                                                        }
                                                        Some(
                                                            types::ui::EntryCursor::CreateVirtual,
                                                        ) => s.ports.order.len().saturating_add(1),
                                                        Some(types::ui::EntryCursor::About) => {
                                                            s.ports.order.len().saturating_add(2)
                                                        }
                                                        None => 0usize,
                                                    },
                                                    types::Page::ModbusDashboard {
                                                        selected_port,
                                                        ..
                                                    }
                                                    | types::Page::ModbusConfig {
                                                        selected_port,
                                                        ..
                                                    }
                                                    | types::Page::ModbusLog {
                                                        selected_port,
                                                        ..
                                                    } => *selected_port,
                                                    _ => 0usize,
                                                };
                                                if sel + 1 < total {
                                                    sel += 1;
                                                } else {
                                                    sel = total.saturating_sub(1);
                                                }
                                                // Write back as Entry cursor
                                                s.page = types::Page::Entry {
                                                    cursor: Some(types::ui::EntryCursor::Com {
                                                        idx: sel,
                                                    }),
                                                };
                                                Ok(())
                                            });
                                            let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                        }
                                        crate::tui::input::Action::MovePrev => {
                                            // Move selection up
                                            let _ = write_status(&app_clone, |s| {
                                                // Read current selection
                                                let mut sel = match &s.page {
                                                    types::Page::Entry { cursor } => match cursor {
                                                        Some(types::ui::EntryCursor::Com {
                                                            idx,
                                                        }) => *idx,
                                                        Some(types::ui::EntryCursor::Refresh) => {
                                                            s.ports.order.len()
                                                        }
                                                        Some(
                                                            types::ui::EntryCursor::CreateVirtual,
                                                        ) => s.ports.order.len().saturating_add(1),
                                                        Some(types::ui::EntryCursor::About) => {
                                                            s.ports.order.len().saturating_add(2)
                                                        }
                                                        None => 0usize,
                                                    },
                                                    types::Page::ModbusDashboard {
                                                        selected_port,
                                                        ..
                                                    }
                                                    | types::Page::ModbusConfig {
                                                        selected_port,
                                                        ..
                                                    }
                                                    | types::Page::ModbusLog {
                                                        selected_port,
                                                        ..
                                                    } => *selected_port,
                                                    _ => 0usize,
                                                };
                                                if sel > 0 {
                                                    sel = sel.saturating_sub(1);
                                                } else {
                                                    sel = 0;
                                                }
                                                s.page = types::Page::Entry {
                                                    cursor: Some(types::ui::EntryCursor::Com {
                                                        idx: sel,
                                                    }),
                                                };
                                                Ok(())
                                            });
                                            let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                        }
                                        crate::tui::input::Action::EnterPage => {
                                            // Enter pressed - if on Entry page:
                                            //  - open ModbusDashboard when a physical port is selected
                                            //  - open About page when the About special entry is selected
                                            if let Ok((sel, ports_order)) =
                                                crate::protocol::status::read_status(
                                                    &app_clone,
                                                    |s| {
                                                        // derive sel from page to be consistent
                                                        let sel = match &s.page {
                                                        types::Page::Entry { cursor } => match cursor {
                                                            Some(types::ui::EntryCursor::Com {
                                                                idx,
                                                            }) => *idx,
                                                            Some(types::ui::EntryCursor::Refresh) => {
                                                                s.ports.order.len()
                                                            }
                                                            Some(
                                                                types::ui::EntryCursor::CreateVirtual,
                                                            ) => s.ports.order.len().saturating_add(1),
                                                            Some(types::ui::EntryCursor::About) => {
                                                                s.ports.order.len().saturating_add(2)
                                                            }
                                                            None => 0usize,
                                                        },
                                                        types::Page::ModbusDashboard {
                                                            selected_port,
                                                            ..
                                                        }
                                                        | types::Page::ModbusConfig {
                                                            selected_port,
                                                            ..
                                                        }
                                                        | types::Page::ModbusLog {
                                                            selected_port,
                                                            ..
                                                        } => *selected_port,
                                                        _ => 0usize,
                                                    };
                                                        Ok((sel, s.ports.order.clone()))
                                                    },
                                                )
                                            {
                                                let ports_len = ports_order.len();
                                                if sel < ports_len {
                                                    // Open ModbusDashboard for the selected port (existing behavior)
                                                    let port_name = ports_order
                                                        .get(sel)
                                                        .cloned()
                                                        .unwrap_or_default();
                                                    let _ = write_status(&app_clone, |s| {
                                                        s.page = types::Page::ModbusDashboard {
                                                            selected_port: sel,
                                                            cursor: 0,
                                                            editing_field: None,
                                                            input_buffer: String::new(),
                                                            edit_choice_index: None,
                                                            edit_confirmed: false,
                                                            master_cursor: 0,
                                                            master_field_selected: false,
                                                            master_field_editing: false,
                                                            master_edit_field: None,
                                                            master_edit_index: None,
                                                            master_input_buffer: String::new(),
                                                            poll_round_index: 0,
                                                            in_flight_reg_index: None,
                                                        };
                                                        s.temporarily.per_port.pending_sync_port =
                                                            Some(port_name.clone());
                                                        Ok(())
                                                    });
                                                    let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                                } else {
                                                    // Selection points into special entries (Refresh, ManualSpecify, About)
                                                    let rel = sel.saturating_sub(ports_len);
                                                    // If About (third special entry) is selected -> open About page
                                                    if rel == 2 {
                                                        let _ = write_status(&app_clone, |s| {
                                                            s.page = types::Page::About {
                                                                view_offset: 0,
                                                            };
                                                            Ok(())
                                                        });
                                                        let _ =
                                                            bus_clone.ui_tx.send(UiToCore::Refresh);
                                                    }
                                                }
                                            }
                                        }
                                        crate::tui::input::Action::LeavePage => {
                                            // Esc pressed - if a subpage is active, go back to Entry page
                                            let _ = write_status(&app_clone, |s| {
                                                // Only change page when currently in a subpage
                                                let subpage_active = matches!(
                                                    s.page,
                                                    types::Page::ModbusConfig { .. }
                                                        | types::Page::ModbusDashboard { .. }
                                                        | types::Page::ModbusLog { .. }
                                                        | types::Page::About { .. }
                                                );
                                                if subpage_active {
                                                    s.page = types::Page::Entry { cursor: None };
                                                }
                                                Ok(())
                                            });
                                            let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                        }
                                        crate::tui::input::Action::TogglePort => {
                                            // Enter pressed - toggle runtime for selected port
                                            if let Ok((sel, ports_order)) =
                                                crate::protocol::status::read_status(
                                                    &app_clone,
                                                    |s| {
                                                        let sel = match &s.page {
                                                        types::Page::Entry { cursor } => match cursor {
                                                            Some(types::ui::EntryCursor::Com {
                                                                idx,
                                                            }) => *idx,
                                                            Some(types::ui::EntryCursor::Refresh) => {
                                                                s.ports.order.len()
                                                            }
                                                            Some(
                                                                types::ui::EntryCursor::CreateVirtual,
                                                            ) => s.ports.order.len().saturating_add(1),
                                                            Some(types::ui::EntryCursor::About) => {
                                                                s.ports.order.len().saturating_add(2)
                                                            }
                                                            None => 0usize,
                                                        },
                                                        types::Page::ModbusDashboard {
                                                            selected_port,
                                                            ..
                                                        }
                                                        | types::Page::ModbusConfig {
                                                            selected_port, ..
                                                        }
                                                        | types::Page::ModbusLog {
                                                            selected_port, ..
                                                        } => *selected_port,
                                                        _ => 0usize,
                                                    };
                                                        Ok((sel, s.ports.order.clone()))
                                                    },
                                                )
                                            {
                                                let ports_len = ports_order.len();
                                                if sel < ports_len {
                                                    if let Some(port_name) =
                                                        ports_order.get(sel).cloned()
                                                    {
                                                        let _ = bus_clone.ui_tx.send(
                                                            UiToCore::ToggleRuntime(port_name),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        crate::tui::input::Action::QuickScan => {
                                            let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                        }
                                        crate::tui::input::Action::None => {
                                            // No global action mapped: consult page-level mapping and handlers.
                                            if let Some(page_act) =
                                                crate::tui::ui::pages::map_key_in_page(
                                                    key, &snapshot,
                                                )
                                            {
                                                match page_act {
                                                    crate::tui::input::Action::MoveNext => {
                                                        let _ = write_status(&app_clone, |s| {
                                                            let special_base = s.ports.order.len();
                                                            let extra_count =
                                                                SpecialEntry::all().len();
                                                            let total = special_base + extra_count;
                                                            let mut sel = match &s.page {
                                                                types::Page::Entry { cursor } => match cursor {
                                                                    Some(types::ui::EntryCursor::Com { idx }) => *idx,
                                                                    Some(types::ui::EntryCursor::Refresh) => s.ports.order.len(),
                                                                    Some(types::ui::EntryCursor::CreateVirtual) => s.ports.order.len().saturating_add(1),
                                                                    Some(types::ui::EntryCursor::About) => s.ports.order.len().saturating_add(2),
                                                                    None => 0usize,
                                                                },
                                                                types::Page::ModbusDashboard { selected_port, .. }
                                                                | types::Page::ModbusConfig { selected_port, .. }
                                                                | types::Page::ModbusLog { selected_port, .. } => *selected_port,
                                                                _ => 0usize,
                                                            };
                                                            if sel + 1 < total {
                                                                sel += 1;
                                                            } else {
                                                                sel = total.saturating_sub(1);
                                                            }
                                                            s.page = types::Page::Entry {
                                                                cursor: Some(
                                                                    types::ui::EntryCursor::Com {
                                                                        idx: sel,
                                                                    },
                                                                ),
                                                            };
                                                            Ok(())
                                                        });
                                                        let _ =
                                                            bus_clone.ui_tx.send(UiToCore::Refresh);
                                                    }
                                                    crate::tui::input::Action::MovePrev => {
                                                        let _ = write_status(&app_clone, |s| {
                                                            let mut sel = match &s.page {
                                                                types::Page::Entry { cursor } => match cursor {
                                                                    Some(types::ui::EntryCursor::Com { idx }) => *idx,
                                                                    Some(types::ui::EntryCursor::Refresh) => s.ports.order.len(),
                                                                    Some(types::ui::EntryCursor::CreateVirtual) => s.ports.order.len().saturating_add(1),
                                                                    Some(types::ui::EntryCursor::About) => s.ports.order.len().saturating_add(2),
                                                                    None => 0usize,
                                                                },
                                                                types::Page::ModbusDashboard { selected_port, .. }
                                                                | types::Page::ModbusConfig { selected_port, .. }
                                                                | types::Page::ModbusLog { selected_port, .. } => *selected_port,
                                                                _ => 0usize,
                                                            };
                                                            if sel > 0 {
                                                                sel = sel.saturating_sub(1);
                                                            } else {
                                                                sel = 0;
                                                            }
                                                            s.page = types::Page::Entry {
                                                                cursor: Some(
                                                                    types::ui::EntryCursor::Com {
                                                                        idx: sel,
                                                                    },
                                                                ),
                                                            };
                                                            Ok(())
                                                        });
                                                        let _ =
                                                            bus_clone.ui_tx.send(UiToCore::Refresh);
                                                    }
                                                    _ => {}
                                                }
                                            } else {
                                                // If page didn't map the key, call raw page handler which may consume it
                                                let _consumed =
                                                    crate::tui::ui::pages::handle_input_in_page(
                                                        key, &snapshot, &bus_clone, &app_clone,
                                                    );
                                                // If consumed, we may want to trigger a refresh
                                                if _consumed {
                                                    let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

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
