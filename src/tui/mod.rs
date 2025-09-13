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
            port::{PortData, PortLogEntry, PortState},
            ui::SpecialEntry,
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
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
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
                // Each listener will watch the PortRuntimeHandle.evt_rx (if runtime exists)
                // and append FrameReceived/FrameSent events into the corresponding PortData.logs.
                // Use a short-lived snapshot to find runtime handles to listen to.
                if let Ok(snapshot) =
                    crate::protocol::status::read_status(app_ref, |s| Ok(s.clone()))
                {
                    for port_name in snapshot.ports.order.iter() {
                        if let Some(pd) = snapshot.ports.map.get(port_name) {
                            if let Some(runtime) = pd.runtime.as_ref() {
                                // Spawn a listener thread for this runtime's evt_rx.
                                // Clone channel/handles needed into thread.
                                let evt_rx = runtime.evt_rx.clone();
                                let app_clone3 = Arc::clone(app_ref);
                                let port_name_clone = port_name.clone();
                                thread::spawn(move || {
                                    // Limit per-port log length
                                    const MAX_LOGS: usize = 2000;
                                    while let Ok(evt) = evt_rx.recv() {
                                        match evt {
                                            crate::protocol::runtime::RuntimeEvent::FrameReceived(b)
                                            | crate::protocol::runtime::RuntimeEvent::FrameSent(b) => {
                                                let now = chrono::Local::now();
                                                let raw = b.iter().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join(" ");
                                                // Best-effort parsed summary: first few bytes hex
                                                let parsed = Some(format!("{} bytes", b.len()));
                                                let entry = PortLogEntry {
                                                    when: now,
                                                    raw,
                                                    parsed,
                                                };
                                                // Append into Status under write_status
                                                let _ = write_status(&app_clone3, |s| {
                                                    if let Some(pdata) = s.ports.map.get_mut(&port_name_clone) {
                                                        pdata.logs.push(entry.clone());
                                                        // Truncate if exceeding max
                                                        if pdata.logs.len() > MAX_LOGS {
                                                            let drop = pdata.logs.len() - MAX_LOGS;
                                                            pdata.logs.drain(0..drop);
                                                        }
                                                        // If auto-scroll is enabled, move selection to last
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
                    if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
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
                            let consumed = crate::tui::ui::pages::handle_input_in_subpage(
                                key, &snapshot, &bus_clone,
                            );

                            if !consumed {
                                // Handle global keys
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
                                                crate::protocol::status::types::Page::Entry { cursor } => match cursor {
                                                    Some(crate::protocol::status::types::ui::EntryCursor::Com { idx }) => *idx,
                                                    Some(crate::protocol::status::types::ui::EntryCursor::Refresh) => s.ports.order.len(),
                                                    Some(crate::protocol::status::types::ui::EntryCursor::CreateVirtual) => s.ports.order.len().saturating_add(1),
                                                    Some(crate::protocol::status::types::ui::EntryCursor::About) => s.ports.order.len().saturating_add(2),
                                                    None => 0usize,
                                                },
                                                crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. }
                                                | crate::protocol::status::types::Page::ModbusConfig { selected_port }
                                                | crate::protocol::status::types::Page::ModbusLog { selected_port, .. } => *selected_port,
                                                _ => 0usize,
                                            };
                                            if sel + 1 < total {
                                                sel += 1;
                                            } else {
                                                sel = total.saturating_sub(1);
                                            }
                                            // Write back as Entry cursor
                                            s.page = crate::protocol::status::types::Page::Entry {
                                                cursor: Some(crate::protocol::status::types::ui::EntryCursor::Com { idx: sel }),
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
                                                crate::protocol::status::types::Page::Entry { cursor } => match cursor {
                                                    Some(crate::protocol::status::types::ui::EntryCursor::Com { idx }) => *idx,
                                                    Some(crate::protocol::status::types::ui::EntryCursor::Refresh) => s.ports.order.len(),
                                                    Some(crate::protocol::status::types::ui::EntryCursor::CreateVirtual) => s.ports.order.len().saturating_add(1),
                                                    Some(crate::protocol::status::types::ui::EntryCursor::About) => s.ports.order.len().saturating_add(2),
                                                    None => 0usize,
                                                },
                                                crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. }
                                                | crate::protocol::status::types::Page::ModbusConfig { selected_port }
                                                | crate::protocol::status::types::Page::ModbusLog { selected_port, .. } => *selected_port,
                                                _ => 0usize,
                                            };
                                            if sel > 0 {
                                                sel = sel.saturating_sub(1);
                                            } else {
                                                sel = 0;
                                            }
                                            s.page = crate::protocol::status::types::Page::Entry {
                                                cursor: Some(crate::protocol::status::types::ui::EntryCursor::Com { idx: sel }),
                                            };
                                            Ok(())
                                        });
                                        let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                    }
                                    crate::tui::input::Action::EnterPage
                                    | crate::tui::input::Action::TogglePort => {
                                        // If selected is a port entry, open subpage and populate form
                                        if let Ok((sel, ports_order)) =
                                            crate::protocol::status::read_status(&app_clone, |s| {
                                                // derive sel from page to be consistent
                                                let sel = match &s.page {
                                                    crate::protocol::status::types::Page::Entry { cursor } => match cursor {
                                                        Some(crate::protocol::status::types::ui::EntryCursor::Com { idx }) => *idx,
                                                        Some(crate::protocol::status::types::ui::EntryCursor::Refresh) => s.ports.order.len(),
                                                        Some(crate::protocol::status::types::ui::EntryCursor::CreateVirtual) => s.ports.order.len().saturating_add(1),
                                                        Some(crate::protocol::status::types::ui::EntryCursor::About) => s.ports.order.len().saturating_add(2),
                                                        None => 0usize,
                                                    },
                                                    crate::protocol::status::types::Page::ModbusDashboard { selected_port, .. }
                                                    | crate::protocol::status::types::Page::ModbusConfig { selected_port }
                                                    | crate::protocol::status::types::Page::ModbusLog { selected_port, .. } => *selected_port,
                                                    _ => 0usize,
                                                };
                                                Ok((sel, s.ports.order.clone()))
                                            })
                                        {
                                            let ports_len = ports_order.len();
                                            if sel < ports_len {
                                                let port_name = ports_order
                                                    .get(sel)
                                                    .cloned()
                                                    .unwrap_or_default();
                                                let _ = write_status(&app_clone, |s| {
                                                    // Open the ModbusDashboard for the selected port.
                                                    s.page = crate::protocol::status::types::Page::ModbusDashboard {
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
                                            }
                                        }
                                    }
                                    crate::tui::input::Action::QuickScan => {
                                        let _ = bus_clone.ui_tx.send(UiToCore::Refresh);
                                    }
                                    _ => {}
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
    crossterm::execute!(stdout, crossterm::terminal::LeaveAlternateScreen)?;
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
        crate::protocol::status::types::Page::ModbusConfig { .. }
            | crate::protocol::status::types::Page::ModbusDashboard { .. }
            | crate::protocol::status::types::Page::ModbusLog { .. }
            | crate::protocol::status::types::Page::About { .. }
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
