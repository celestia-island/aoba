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
    protocol::status::Status,
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
        let _ = crate::protocol::status::status_rw::write_status(&app, |g| {
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
        let app_clone = Arc::clone(&app);
        let core_tx_clone = core_tx.clone();
        thread::spawn(move || {
            let mut last_scan_time = std::time::Instant::now();
            let scan_interval = Duration::from_secs(5); // Scan every 5 seconds
            let mut polling_paused = false;
            
            loop {
                let now = std::time::Instant::now();
                
                // Handle commands coming from UI
                while let Ok(msg) = ui_rx.try_recv() {
                    match msg {
                        UiToCore::Quit => {
                            log::info!("[CORE] Received quit signal");
                            return;
                        }
                        UiToCore::Refresh => {
                            log::debug!("[CORE] Refresh requested");
                            perform_port_scan(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::PausePolling => {
                            log::debug!("[CORE] Pause polling requested");
                            polling_paused = true;
                        }
                        UiToCore::ResumePolling => {
                            log::debug!("[CORE] Resume polling requested");
                            polling_paused = false;
                        }
                        UiToCore::NavigateUp => {
                            log::debug!("[CORE] Navigate up requested");
                            handle_navigation(&app_clone, -1);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::NavigateDown => {
                            log::debug!("[CORE] Navigate down requested");
                            handle_navigation(&app_clone, 1);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::EnterSelection => {
                            log::debug!("[CORE] Enter selection requested");
                            handle_enter_selection(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::GoBack => {
                            log::debug!("[CORE] Go back requested");
                            handle_go_back(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::ScanPorts => {
                            log::debug!("[CORE] Force port scan requested");
                            perform_port_scan(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::StartEdit => {
                            log::debug!("[CORE] Start edit requested");
                            handle_start_edit(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::CancelEdit => {
                            log::debug!("[CORE] Cancel edit requested");
                            handle_cancel_edit(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::InputChar(c) => {
                            log::debug!("[CORE] Input char: {}", c);
                            handle_input_char(&app_clone, c);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::Backspace => {
                            log::debug!("[CORE] Backspace requested");
                            handle_backspace(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                        UiToCore::ConfirmEdit => {
                            log::debug!("[CORE] Confirm edit requested");
                            handle_confirm_edit(&app_clone);
                            let _ = core_tx_clone.send(CoreToUi::Refreshed);
                        }
                    }
                }

                // Perform periodic port scanning if not paused
                if !polling_paused && now.duration_since(last_scan_time) >= scan_interval {
                    log::debug!("[CORE] Performing periodic port scan");
                    perform_port_scan(&app_clone);
                    last_scan_time = now;
                    let _ = core_tx_clone.send(CoreToUi::Refreshed);
                }

                // Send periodic tick
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

                        // Route input to appropriate page handler
                        if let Ok(app_read) = app_clone.read() {
                            let consumed = crate::tui::ui::pages::handle_input_in_subpage(
                                key, &app_read, &bus_clone,
                            );

                            if !consumed {
                                // Handle global keys
                                match key.code {
                                    crossterm::event::KeyCode::Char('q')
                                    | crossterm::event::KeyCode::Char('Q') => {
                                        let _ = bus_clone.ui_tx.send(UiToCore::Quit);
                                        break;
                                    }
                                    crossterm::event::KeyCode::Char('r') => {
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

        // Render UI - only read from Status, never mutate
        terminal.draw(|f| {
            if let Ok(app_read) = app.read() {
                render_ui_readonly(f, &app_read);
            }
        })?;
    }

    terminal.clear()?;
    Ok(())
}

/// Render UI function that only reads from Status (immutable reference)
fn render_ui_readonly(f: &mut Frame, app: &Status) {
    let area = f.area();
    let bottom_len = if app.page.error.is_some() || app.page.subpage_active {
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

    if app.page.mode_overlay_active {
        let idx = app.page.mode_overlay_index.as_usize().min(1);
        crate::tui::ui::components::mode_selector::render_mode_selector(f, idx);
    }
}

/// Perform port scanning and update the status
fn perform_port_scan(app: &Arc<RwLock<Status>>) {
    use crate::protocol::{status::status_rw::write_status, tty::available_ports_enriched};
    
    let scan_result = std::panic::catch_unwind(|| {
        let ports = available_ports_enriched();
        let scan_info = format!("Found {} ports", ports.len());
        (ports, scan_info)
    });

    match scan_result {
        Ok((ports, scan_info)) => {
            let _ = write_status(app, |status| {
                // Update ports list - for now, just update scan info
                // TODO: Implement proper port list update once structure is clarified
                log::debug!("[CORE] Found {} ports during scan", ports.len());

                // Update scan info
                status.scan.last_scan_time = Some(chrono::Local::now());
                status.scan.last_scan_info = scan_info.clone();

                Ok(())
            });
        }
        Err(_) => {
            let _ = write_status(app, |status| {
                status.scan.last_scan_time = Some(chrono::Local::now());
                status.scan.last_scan_info = "ERROR: Port scan failed".to_string();
                Ok(())
            });
        }
    }
}

/// Handle navigation (up/down) in the current view
fn handle_navigation(app: &Arc<RwLock<Status>>, direction: i32) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if status.page.subpage_active {
            // Handle navigation within subpages
            if let Some(ref mut form) = status.page.subpage_form {
                let max_fields = 6; // 0-5: global_interval, master_passive, baud, data_bits, parity, stop_bits
                if direction > 0 {
                    // Move down
                    form.cursor = (form.cursor + 1) % max_fields;
                } else {
                    // Move up
                    form.cursor = if form.cursor == 0 {
                        max_fields - 1
                    } else {
                        form.cursor - 1
                    };
                }
                log::debug!("[CORE] Config navigation: cursor now at {}", form.cursor);
            }
        } else {
            // Handle navigation in main entry list
            let total_items = status.ports.list.len() + 2; // +2 for refresh and about
            if total_items > 0 {
                if direction > 0 {
                    // Move down
                    status.page.selected = (status.page.selected + 1) % total_items;
                } else {
                    // Move up
                    status.page.selected = if status.page.selected == 0 {
                        total_items - 1
                    } else {
                        status.page.selected - 1
                    };
                }
            }
        }
        Ok(())
    });
}

/// Handle entering the selected item
fn handle_enter_selection(app: &Arc<RwLock<Status>>) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if status.page.subpage_active {
            // Already in a subpage, maybe toggle editing or perform action
            log::debug!("[CORE] Enter selection in subpage not yet implemented");
        } else {
            // Enter subpage for selected port
            let total_ports = status.ports.list.len();
            if status.page.selected < total_ports {
                // Selected a port, enter its subpage
                status.page.subpage_active = true;
                
                // Initialize subpage form if needed
                if status.page.subpage_form.is_none() {
                    status.page.subpage_form = Some(crate::protocol::status::SubpageForm {
                        registers: Vec::new(),
                        master_cursor: 0,
                        master_field_selected: false,
                        master_field_editing: false,
                        master_edit_field: None,
                        master_edit_index: None,
                        master_input_buffer: String::new(),
                        cursor: 0,
                        loop_enabled: false,
                        master_passive: None,
                        editing: false,
                        editing_field: None,
                        input_buffer: String::new(),
                        edit_choice_index: None,
                        edit_confirmed: false,
                        baud: 9600,
                        parity: serialport::Parity::None,
                        data_bits: 8,
                        stop_bits: 1,
                        global_interval_ms: 1000,
                        global_timeout_ms: 5000,
                    });
                }
                
                log::debug!("[CORE] Entered subpage for port {}", status.page.selected);
            } else {
                // Selected special item (refresh/about)
                let special_idx = status.page.selected - total_ports;
                if special_idx == 0 {
                    // Refresh action
                    perform_port_scan(app);
                } else if special_idx == 1 {
                    // About page
                    status.page.subpage_active = true;
                    log::debug!("[CORE] Entered about page");
                }
            }
        }
        Ok(())
    });
}

/// Handle going back to the previous level
fn handle_go_back(app: &Arc<RwLock<Status>>) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if status.page.subpage_active {
            // Exit subpage
            status.page.subpage_active = false;
            
            // Clear editing state if any
            if let Some(ref mut form) = status.page.subpage_form {
                form.editing = false;
                form.input_buffer.clear();
            }
            
            log::debug!("[CORE] Exited subpage");
        }
        Ok(())
    });
}

/// Handle starting edit mode for the currently selected field
fn handle_start_edit(app: &Arc<RwLock<Status>>) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if let Some(ref mut form) = status.page.subpage_form {
            if form.editing {
                // Already editing, so this Enter press should confirm the edit
                let success = apply_config_change(form);
                if success {
                    form.editing = false;
                    form.input_buffer.clear();
                    log::info!("[CORE] Applied configuration change for field {}", form.cursor);
                } else {
                    log::warn!("[CORE] Failed to apply configuration change for field {}", form.cursor);
                }
            } else {
                // Not editing, so start editing
                form.editing = true;
                form.input_buffer.clear();
                
                // Pre-populate buffer with current value for easier editing
                match form.cursor {
                    0 => form.input_buffer = form.global_interval_ms.to_string(),
                    1 => {
                        // Master passive - toggle directly instead of text editing
                        form.master_passive = match form.master_passive {
                            Some(true) => Some(false),
                            Some(false) => None,
                            None => Some(true),
                        };
                        form.editing = false; // Don't enter text editing mode for toggle
                        log::info!("[CORE] Toggled master passive to: {:?}", form.master_passive);
                        return Ok(());
                    }
                    2 => form.input_buffer = form.baud.to_string(),
                    3 => form.input_buffer = form.data_bits.to_string(),
                    4 => {
                        // For parity, show first letter
                        form.input_buffer = match form.parity {
                            serialport::Parity::None => "n".to_string(),
                            serialport::Parity::Even => "e".to_string(),
                            serialport::Parity::Odd => "o".to_string(),
                        };
                    }
                    5 => form.input_buffer = form.stop_bits.to_string(),
                    _ => {}
                }
                
                log::debug!("[CORE] Started editing field {} with buffer: '{}'", form.cursor, form.input_buffer);
            }
        }
        Ok(())
    });
}

/// Handle canceling edit mode
fn handle_cancel_edit(app: &Arc<RwLock<Status>>) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if let Some(ref mut form) = status.page.subpage_form {
            form.editing = false;
            form.input_buffer.clear();
            log::debug!("[CORE] Canceled editing");
        }
        Ok(())
    });
}

/// Handle input character during editing
fn handle_input_char(app: &Arc<RwLock<Status>>, c: char) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if let Some(ref mut form) = status.page.subpage_form {
            if form.editing {
                match form.cursor {
                    0 | 2 | 3 | 5 => {
                        // Numeric fields: global_interval, baud, data_bits, stop_bits
                        if c.is_ascii_digit() {
                            form.input_buffer.push(c);
                        }
                    }
                    4 => {
                        // Parity field: accept n, e, o
                        if c == 'n' || c == 'e' || c == 'o' {
                            form.input_buffer.clear();
                            form.input_buffer.push(c);
                        }
                    }
                    _ => {}
                }
                log::debug!("[CORE] Input buffer now: '{}'", form.input_buffer);
            }
        }
        Ok(())
    });
}

/// Handle backspace during editing
fn handle_backspace(app: &Arc<RwLock<Status>>) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if let Some(ref mut form) = status.page.subpage_form {
            if form.editing && !form.input_buffer.is_empty() {
                form.input_buffer.pop();
                log::debug!("[CORE] Input buffer after backspace: '{}'", form.input_buffer);
            }
        }
        Ok(())
    });
}

/// Handle confirming/applying the current edit using write_status
fn handle_confirm_edit(app: &Arc<RwLock<Status>>) {
    use crate::protocol::status::status_rw::write_status;
    
    let _ = write_status(app, |status| {
        if let Some(ref mut form) = status.page.subpage_form {
            if form.editing {
                let success = apply_config_change(form);
                if success {
                    form.editing = false;
                    form.input_buffer.clear();
                    log::info!("[CORE] Applied configuration change for field {}", form.cursor);
                } else {
                    log::warn!("[CORE] Failed to apply configuration change for field {}", form.cursor);
                }
            }
        }
        Ok(())
    });
}

/// Apply configuration change to the form fields using write_status pattern
fn apply_config_change(form: &mut crate::protocol::status::SubpageForm) -> bool {
    if form.input_buffer.is_empty() {
        return false;
    }
    
    match form.cursor {
        0 => {
            // Global interval
            if let Ok(val) = form.input_buffer.parse::<u64>() {
                if val > 0 && val <= 60000 { // reasonable range: 1ms to 60s
                    form.global_interval_ms = val;
                    return true;
                }
            }
        }
        1 => {
            // Master passive - toggle field, no direct text input
            form.master_passive = match form.master_passive {
                Some(true) => Some(false),
                Some(false) => None,
                None => Some(true),
            };
            return true;
        }
        2 => {
            // Baud rate
            if let Ok(val) = form.input_buffer.parse::<u32>() {
                if val >= 300 && val <= 230400 { // common baud rate range
                    form.baud = val;
                    return true;
                }
            }
        }
        3 => {
            // Data bits
            if let Ok(val) = form.input_buffer.parse::<u8>() {
                if val >= 5 && val <= 8 {
                    form.data_bits = val;
                    return true;
                }
            }
        }
        4 => {
            // Parity
            match form.input_buffer.as_str() {
                "n" => {
                    form.parity = serialport::Parity::None;
                    return true;
                }
                "e" => {
                    form.parity = serialport::Parity::Even;
                    return true;
                }
                "o" => {
                    form.parity = serialport::Parity::Odd;
                    return true;
                }
                _ => {}
            }
        }
        5 => {
            // Stop bits
            if let Ok(val) = form.input_buffer.parse::<u8>() {
                if val == 1 || val == 2 {
                    form.stop_bits = val;
                    return true;
                }
            }
        }
        _ => {}
    }
    
    false
}
