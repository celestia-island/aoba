pub mod input;
pub mod ui;
pub mod utils; // Newly added helpers for form editing

use anyhow::Result;
use std::{
    io::{self, Stdout},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::i18n::lang;
use crate::protocol::status::AppMode;
use ratatui::{backend::CrosstermBackend, prelude::*};

// Number of base (non-register) configurable fields in subpage forms. Keep in sync with
// The rendering order in `src/tui/ui/components/config_panel.rs`.
const BASE_FIELD_COUNT: usize = 8;

use crate::{
    protocol::status::{InputMode, LogEntry, Status},
    tui::{
        input::{map_key, Action},
        utils::constants::LOG_PAGE_JUMP,
    },
};
use serialport::Parity;

fn is_log_tab(app: &Status) -> bool {
    app.subpage_active && app.subpage_tab_index == 2
}

/// Recompute log viewport (bottom anchored) after `log_selected` potentially changed.
fn adjust_log_view(app: &mut Status, term_height: u16) {
    app.adjust_log_view(term_height);
}

/// Convenience wrapper around terminal draw + locking to reduce repetition.
fn redraw(terminal: &mut Terminal<CrosstermBackend<&mut Stdout>>, app: &Arc<Mutex<Status>>) {
    let _ = terminal.draw(|f| {
        if let Ok(mut g) = app.lock() {
            crate::tui::ui::render_ui(f, &mut g);
        }
    });
}

pub fn start() -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");

    // Setup terminal
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = Arc::new(Mutex::new(Status::new()));

    // For manual testing: if AOBA_TUI_FORCE_ERROR is set, pre-populate an error to display
    if std::env::var("AOBA_TUI_FORCE_ERROR").is_ok() {
        if let Ok(mut guard) = app.lock() {
            guard.set_error("demo forced error: AOBA_TUI_FORCE_ERROR");
        }
    }

    // Unified core worker thread: handles all non-UI periodic logic (port refresh, register polling, draining events) and communicates via the bus.
    use crate::tui::utils::bus::{Bus, CoreToUi, UiToCore};
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>(); // core -> ui
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>(); // ui -> core
    let bus = Bus::new(core_rx, ui_tx.clone());

    // Core thread
    {
        let app_clone = Arc::clone(&app);
        thread::spawn(move || {
            let mut last_full_scan = std::time::Instant::now();
            let mut last_ports_refresh = std::time::Instant::now();
            loop {
                // Handle commands coming from UI
                while let Ok(msg) = ui_rx.try_recv() {
                    match msg {
                        UiToCore::Refresh => {
                            if let Ok(mut guard) = app_clone.lock() {
                                guard.refresh();
                            }
                            let _ = core_tx.send(CoreToUi::Refreshed);
                        }
                        UiToCore::Quit => {
                            return;
                        }
                        UiToCore::PausePolling => {
                            if let Ok(mut guard) = app_clone.lock() {
                                guard.pause_and_reset_slave_listen();
                            }
                            let _ = core_tx.send(CoreToUi::Refreshed);
                        }
                        UiToCore::ResumePolling => {
                            if let Ok(mut guard) = app_clone.lock() {
                                guard.resume_slave_listen();
                            }
                            let _ = core_tx.send(CoreToUi::Refreshed);
                        }
                    }
                }

                // Lightweight port list refresh (every 1s)
                if last_ports_refresh.elapsed() >= Duration::from_millis(1000) {
                    if let Ok(mut guard) = app_clone.lock() {
                        guard.refresh_ports_only();
                    }
                    last_ports_refresh = std::time::Instant::now();
                    let _ = core_tx.send(CoreToUi::Refreshed);
                }

                // Full device scan (includes external commands) at lower frequency (e.g. every 15s) to avoid UI stalls
                if last_full_scan.elapsed() >= Duration::from_secs(15) {
                    if let Ok(mut guard) = app_clone.lock() {
                        guard.refresh();
                    }
                    last_full_scan = std::time::Instant::now();
                    let _ = core_tx.send(CoreToUi::Refreshed);
                }

                // Drive polling + sync runtime configs + drain events (keep lock short)
                if let Ok(mut guard) = app_clone.lock() {
                    guard.sync_runtime_configs();
                    guard.drive_slave_polling();
                    guard.drain_runtime_events();
                    guard.tick_spinner();
                }
                let _ = core_tx.send(CoreToUi::Tick);
                thread::sleep(Duration::from_millis(40));
            }
        });
    }

    let res = run_app(&mut terminal, Arc::clone(&app), bus);

    // Restore terminal
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::LeaveAlternateScreen)?;
    crossterm::terminal::disable_raw_mode()?;

    res
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<&mut Stdout>>,
    app: Arc<Mutex<Status>>,
    bus: crate::tui::utils::bus::Bus,
) -> Result<()> {
    loop {
        // First try to receive a notification from core thread (short timeout) to reduce busy waiting
        let _ = bus.core_rx.recv_timeout(Duration::from_millis(50));
        // Rendering only (read state)
        if let Ok(mut guard) = app.lock() {
            terminal.draw(|f| crate::tui::ui::render_ui(f, &mut guard))?;
        }

        // Poll for input
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                if key.kind != crossterm::event::KeyEventKind::Press {
                    continue; // Ignore non-initial key press (repeat / release)
                }
                // Main keyboard event handling entry
                let lock = app.lock();
                // Removed unused `_is_editing` binding (no side-effects)

                // Mode overlay handling
                let overlay_active = lock
                    .as_ref()
                    .map(|g| g.mode_overlay_active)
                    .unwrap_or(false);
                if overlay_active {
                    use crossterm::event::KeyCode as KC;
                    // Release read lock before acquiring mutable
                    drop(lock);
                    if let Ok(mut guard) = app.lock() {
                        match key.code {
                            KC::Esc => {
                                guard.mode_overlay_active = false;
                            }
                            KC::Tab => {
                                guard.mode_overlay_index = (guard.mode_overlay_index + 1) % 2;
                            }
                            KC::Enter => {
                                let sel = if guard.mode_overlay_index % 2 == 0 {
                                    AppMode::Modbus
                                } else {
                                    AppMode::Mqtt
                                };
                                if guard.app_mode != sel {
                                    guard.app_mode = sel;
                                    guard.save_current_port_state();
                                }
                                guard.mode_overlay_active = false;
                            }
                            _ => {}
                        }
                        guard.clear_error();
                        // Redraw immediately
                        terminal.draw(|f| crate::tui::ui::render_ui(f, &mut guard))?;
                    }
                    continue;
                } else {
                    drop(lock);
                }

                // Re-evaluate editing after potential selector handling
                let is_editing = match app.lock() {
                    Ok(g) => g.subpage_form.as_ref().map(|f| f.editing).unwrap_or(false),
                    Err(_) => false,
                };

                if is_editing {
                    use crossterm::event::KeyCode as KC;
                    if let Ok(mut guard) = app.lock() {
                        let mut pending_error: Option<String> = None;
                        if let Some(form) = guard.subpage_form.as_mut() {
                            match key.code {
                                KC::Char(c) => {
                                    // For Baud field only accept digits; other fields accept any char
                                    if let Some(field) = &form.editing_field {
                                        match field {
                                            crate::protocol::status::EditingField::Baud => {
                                                if c.is_ascii_digit() {
                                                    form.input_buffer.push(c);
                                                }
                                            }
                                            _ => form.input_buffer.push(c),
                                        }
                                    } else {
                                        // Pre-confirm case: assume Baud custom pending -> accept digits only
                                        if c.is_ascii_digit() {
                                            form.input_buffer.push(c);
                                        }
                                    }
                                }
                                KC::Backspace => {
                                    form.input_buffer.pop();
                                }
                                KC::Left | KC::Right => {
                                    // Try to interpret and adjust current field numerically or toggle parity
                                    if let Some(field) = &form.editing_field {
                                        let dir: i64 = match key.code {
                                            KC::Left => -1,
                                            KC::Right => 1,
                                            _ => 0,
                                        };
                                        match field {
                                            crate::protocol::status::EditingField::Loop => {
                                                // Commit toggle immediately
                                                // Nothing special to validate here.
                                            }
                                            crate::protocol::status::EditingField::GlobalInterval => {
                                                let step: i64 = if dir > 0 { 100 } else { -100 };
                                                let mut next = form.global_interval_ms as i64 + step;
                                                if next < 100 { next = 100; }
                                                form.global_interval_ms = next as u64;
                                            }
                                            crate::protocol::status::EditingField::GlobalTimeout => {
                                                let step: i64 = if dir > 0 { 100 } else { -100 };
                                                let mut next = form.global_timeout_ms as i64 + step;
                                                if next < 100 { next = 100; }
                                                form.global_timeout_ms = next as u64;
                                            }
                                            crate::protocol::status::EditingField::Baud => {
                                                let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                                                let custom_idx = presets.len();
                                                // Initialize edit_choice_index if missing
                                                if form.edit_choice_index.is_none() {
                                                    let idx = presets.iter().position(|&p| p == form.baud).unwrap_or(custom_idx);
                                                    form.edit_choice_index = Some(idx);
                                                }
                                                if let Some(mut idx) = form.edit_choice_index {
                                                    if dir > 0 {
                                                        // Move right
                                                        if idx >= custom_idx {
                                                            idx = 0;
                                                        } else {
                                                            idx += 1;
                                                        }
                                                    } else {
                                                        // Move left
                                                        if idx == 0 {
                                                            idx = custom_idx;
                                                        } else {
                                                            idx -= 1;
                                                        }
                                                    }
                                                    form.edit_choice_index = Some(idx);
                                                    // If moved to preset, clear buffer and update baud preview
                                                    if idx < presets.len() {
                                                        form.input_buffer.clear();
                                                        form.baud = presets[idx];
                                                    }
                                                }
                                            }
                                            crate::protocol::status::EditingField::StopBits => {
                                                // Cycle among 1, 2
                                                let options = [1, 2];
                                                let cur_idx = options.iter().position(|&v| v == form.stop_bits).unwrap_or(0);
                                                let next = if dir > 0 { (cur_idx + 1) % options.len() } else { (cur_idx + options.len() - 1) % options.len() };
                                                form.stop_bits = options[next];
                                            }
                                            crate::protocol::status::EditingField::Parity => {
                                                // Cycle parity options
                                                let options = [Parity::None, Parity::Even, Parity::Odd];
                                                let idx = options.iter().position(|&p| p == form.parity).unwrap_or(0);
                                                let next = if dir > 0 { (idx + 1) % options.len() } else { (idx + options.len() - 1) % options.len() };
                                                form.parity = options[next];
                                            }
                                            crate::protocol::status::EditingField::DataBits => {
                                                let options = [5u8, 6u8, 7u8, 8u8];
                                                let idx = options.iter().position(|&d| d == form.data_bits).unwrap_or(3);
                                                let next = if dir > 0 { (idx + 1) % options.len() } else { (idx + options.len() - 1) % options.len() };
                                                form.data_bits = options[next];
                                            }
                                            crate::protocol::status::EditingField::RegisterField { idx, field } => {
                                                if let Some(reg) = form.registers.get_mut(*idx) {
                                                    match field {
                                                        crate::protocol::status::RegisterField::SlaveId => {
                                                            let mut new = (reg.slave_id as i64).saturating_add(dir);
                                                            if new < 1 { new = 1; }
                                                            if new > u8::MAX as i64 { new = u8::MAX as i64; }
                                                            reg.slave_id = new as u8;
                                                        }
                                                        crate::protocol::status::RegisterField::Mode => {
                                                            use crate::protocol::status::RegisterMode;
                                                            let current = reg.mode as u8 as i64;
                                                            let new_raw = (current).saturating_add(dir);
                                                            // Wrap within 1..=4
                                                            let mut val = if new_raw < 1 { 4 } else if new_raw > 4 { 1 } else { new_raw } as u8;
                                                            if !(1..=4).contains(&val) { val = 1; }
                                                            reg.mode = RegisterMode::from_u8(val);
                                                        }
                                                        crate::protocol::status::RegisterField::Address => {
                                                            let new = (reg.address as i64).saturating_add(dir);
                                                            if new >= 0 && new <= u16::MAX as i64 {
                                                                reg.address = new as u16;
                                                            }
                                                        }
                                                        crate::protocol::status::RegisterField::Length => {
                                                            let new = (reg.length as i64).saturating_add(dir);
                                                            if new >= 0 && new <= u16::MAX as i64 {
                                                                reg.length = new as u16;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                KC::Enter => {
                                    if let Some(crate::protocol::status::EditingField::Baud) =
                                        &form.editing_field
                                    {
                                        let presets: [u32; 8] =
                                            [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                                        let custom_idx = presets.len();
                                        // Compute current index from edit_choice_index or derive from baud
                                        let cur = form.edit_choice_index.unwrap_or_else(|| {
                                            presets
                                                .iter()
                                                .position(|&p| p == form.baud)
                                                .unwrap_or(custom_idx)
                                        });
                                        if cur == custom_idx && !form.edit_confirmed {
                                            // Enter deeper confirmed edit stage
                                            form.edit_confirmed = true;
                                            form.editing = true;
                                            // Keep input_buffer as is (user may have typed)
                                            continue;
                                        }
                                    }
                                    let mut commit_success = true;
                                    if let Some(field) = &form.editing_field {
                                        match field {
                                            crate::protocol::status::EditingField::Baud => {
                                                let presets: [u32; 8] = [
                                                    1200, 2400, 4800, 9600, 19200, 38400, 57600,
                                                    115200,
                                                ];
                                                if let Some(idx) = form.edit_choice_index {
                                                    if idx < presets.len() {
                                                        form.baud = presets[idx];
                                                    } else {
                                                        // Custom: must parse and validate [1200..=2_000_000]
                                                        if !form.input_buffer.is_empty() {
                                                            if let Ok(v) =
                                                                form.input_buffer.parse::<u32>()
                                                            {
                                                                if (1200..=2_000_000).contains(&v) {
                                                                    form.baud = v;
                                                                } else {
                                                                    pending_error = Some(
                                                                        lang()
                                                                            .protocol
                                                                            .modbus
                                                                            .invalid_baud_range
                                                                            .clone(),
                                                                    );
                                                                    commit_success = false;
                                                                }
                                                            } else {
                                                                commit_success = false;
                                                            }
                                                        } else {
                                                            commit_success = false;
                                                        }
                                                    }
                                                } else {
                                                    // Fallback: if buffer present try parse and validate
                                                    if !form.input_buffer.is_empty() {
                                                        if let Ok(v) =
                                                            form.input_buffer.parse::<u32>()
                                                        {
                                                            if (1200..=2_000_000).contains(&v) {
                                                                form.baud = v;
                                                            } else {
                                                                pending_error = Some(
                                                                    lang()
                                                                        .protocol
                                                                        .modbus
                                                                        .invalid_baud_range
                                                                        .clone(),
                                                                );
                                                                commit_success = false;
                                                            }
                                                        } else {
                                                            commit_success = false;
                                                        }
                                                    } else {
                                                        commit_success = false;
                                                    }
                                                }
                                            }
                                            crate::protocol::status::EditingField::GlobalInterval => {
                                                if !form.input_buffer.is_empty() {
                                                    if let Ok(v) = form.input_buffer.parse::<u64>() { form.global_interval_ms = v.max(100); } else { commit_success = false; }
                                                }
                                            }
                                            crate::protocol::status::EditingField::GlobalTimeout => {
                                                if !form.input_buffer.is_empty() {
                                                    if let Ok(v) = form.input_buffer.parse::<u64>() { form.global_timeout_ms = v.max(100); } else { commit_success = false; }
                                                }
                                            }
                                            crate::protocol::status::EditingField::Loop | crate::protocol::status::EditingField::Parity | crate::protocol::status::EditingField::StopBits | crate::protocol::status::EditingField::DataBits | crate::protocol::status::EditingField::RegisterField { .. } => {}
                                        }
                                    }
                                    // Exit editing only when commit succeeded
                                    if commit_success {
                                        form.input_buffer.clear();
                                        form.editing_field = None;
                                        form.editing = false;
                                        form.edit_choice_index = None;
                                        form.edit_confirmed = false;
                                    } else {
                                        // Keep the buffer so user can edit; pending_error will be applied after borrow ends
                                    }
                                }
                                KC::Esc => {
                                    // Cancel current field editing (revert input buffer)
                                    form.input_buffer.clear();
                                    form.editing_field = None;
                                    form.editing = false;
                                    form.edit_choice_index = None;
                                    form.edit_confirmed = false;
                                }
                                _ => {}
                            }
                        }
                        if let Some(msg) = pending_error {
                            guard.set_error(msg);
                        } else {
                            guard.clear_error();
                        }
                    }
                    continue;
                }

                if let Ok(mut guard) = app.lock() {
                    if is_log_tab(&guard) {
                        // Communication log subpage: allow Enter OR 'i' to begin editing the input box.
                        use crossterm::event::KeyCode as KC;
                        // If currently in input editing mode, consume characters / backspace / enter / esc here
                        if guard.input_editing {
                            match key.code {
                                KC::Char(c) => {
                                    if guard.input_mode == InputMode::Ascii {
                                        guard.input_buffer.push(c);
                                    } else {
                                        // Hex mode: accept hex digits only (ignore other chars)
                                        if c.is_ascii_hexdigit() || c.is_whitespace() {
                                            guard.input_buffer.push(c);
                                        }
                                    }
                                }
                                KC::Backspace => {
                                    guard.input_buffer.pop();
                                }
                                KC::Enter => {
                                    // Send: append as raw log entry; mark as parsed with rw = "W" so the UI shows the send label.
                                    let parsed = crate::protocol::status::ParsedRequest {
                                        origin: "local-input".to_string(),
                                        rw: "W".to_string(),
                                        command: "manual".to_string(),
                                        slave_id: 0,
                                        address: 0,
                                        length: 0,
                                    };
                                    let entry = LogEntry {
                                        when: chrono::Local::now(),
                                        raw: guard.input_buffer.clone(),
                                        parsed: Some(parsed),
                                    };
                                    guard.append_log(entry);
                                    guard.input_buffer.clear();
                                    guard.input_editing = false;
                                }
                                KC::Esc => {
                                    guard.input_buffer.clear();
                                    guard.input_editing = false;
                                }
                                _ => {}
                            }
                            guard.clear_error();
                            // Force redraw so input buffer appears immediately
                            drop(guard);
                            redraw(terminal, &app);
                            continue; // Consumed
                        } else {
                            // Not editing: allow quick toggles for edit / mode
                            match key.code {
                                KC::Enter | KC::Char('i') => {
                                    guard.input_editing = true;
                                    guard.clear_error();
                                    drop(guard);
                                    redraw(terminal, &app);
                                    continue;
                                }
                                KC::Char('m') => {
                                    guard.input_mode = match guard.input_mode {
                                        InputMode::Ascii => InputMode::Hex,
                                        InputMode::Hex => InputMode::Ascii,
                                    };
                                    guard.clear_error();
                                    drop(guard);
                                    redraw(terminal, &app);
                                    continue;
                                }
                                KC::Up | KC::Char('k') => {
                                    let total = guard.logs.len();
                                    if total > 0 {
                                        if guard.log_selected == 0 {
                                            guard.log_selected = total - 1;
                                        } else {
                                            guard.log_selected -= 1;
                                        }
                                        let term_h =
                                            terminal.size().map(|r| r.height).unwrap_or(24);
                                        adjust_log_view(&mut guard, term_h);
                                    }
                                    guard.clear_error();
                                    drop(guard);
                                    redraw(terminal, &app);
                                    continue;
                                }
                                KC::Char('c') => {
                                    // Require double-press to clear logs: first press sets a pending flag
                                    if !guard.log_clear_pending {
                                        guard.log_clear_pending = true;
                                    } else {
                                        // Second press: perform clear
                                        guard.logs.clear();
                                        guard.log_selected = 0;
                                        guard.log_view_offset = 0;
                                        guard.log_auto_scroll = true;
                                        guard.log_clear_pending = false;
                                        guard.save_current_port_state();
                                    }
                                    guard.clear_error();
                                    drop(guard);
                                    redraw(terminal, &app);
                                    continue;
                                }
                                KC::Down | KC::Char('j') => {
                                    let total = guard.logs.len();
                                    if total > 0 {
                                        guard.log_selected = (guard.log_selected + 1) % total;
                                        let term_h =
                                            terminal.size().map(|r| r.height).unwrap_or(24);
                                        adjust_log_view(&mut guard, term_h);
                                    }
                                    guard.clear_error();
                                    drop(guard);
                                    redraw(terminal, &app);
                                    continue;
                                }
                                _ => {}
                            }
                        }
                    } else {
                        // Global mode cycle (m) when not in log tab quick-toggle context
                        use crossterm::event::KeyCode as KC;
                        if key.code == KC::Char('m') {
                            // Open overlay instead of immediate cycle
                            guard.mode_overlay_active = true;
                            // Sync overlay index to current mode
                            guard.mode_overlay_index = match guard.app_mode {
                                AppMode::Modbus => 0,
                                AppMode::Mqtt => 1,
                            };
                            guard.clear_error();
                            drop(guard);
                            redraw(terminal, &app);
                            continue;
                        }
                    }
                }

                // Subpage first chance
                if let Ok(mut guard) = app.lock() {
                    if crate::tui::ui::pages::handle_key_in_subpage(key, &mut guard, &bus) {
                        guard.clear_error();
                        continue; // Consumed by subpage
                    }
                }

                {
                    // Try page-level mapping first (inner pages can override), fall back to global mapping
                    let mut action_opt: Option<Action> = None;
                    if let Ok(guard) = app.lock() {
                        action_opt = crate::tui::ui::pages::map_key_in_page(key, &guard);
                    }
                    if action_opt.is_none() {
                        match map_key(key.code) {
                            Action::None => {}
                            a => action_opt = Some(a),
                        }
                    }

                    if let Some(action) = action_opt {
                        match action {
                            Action::Quit => {
                                if let Ok(guard) = app.lock() {
                                    let in_editing = guard
                                        .subpage_form
                                        .as_ref()
                                        .map(|f| f.editing)
                                        .unwrap_or(false);
                                    let allowed = !guard.subpage_active && !in_editing;
                                    if allowed {
                                        let _ =
                                            bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Quit);
                                        break;
                                    } else {
                                        // Silently ignore quit when not allowed (do not show message)
                                    }
                                } else {
                                    log::error!("[TUI] failed to lock app for Quit check");
                                }
                            }
                            Action::LeavePage => {
                                if let Ok(mut guard) = app.lock() {
                                    if guard.subpage_active {
                                        guard.subpage_active = false;
                                    }
                                    guard.clear_error();
                                } else {
                                    log::error!("[TUI] failed to lock app");
                                }
                            }
                            Action::EnterPage => {
                                if let Ok(mut guard) = app.lock() {
                                    let state = guard
                                        .port_states
                                        .get(guard.selected)
                                        .cloned()
                                        .unwrap_or(crate::protocol::status::PortState::Free);
                                    // If selected is a real port occupied by this app, open subpage form.
                                    if state == crate::protocol::status::PortState::OccupiedByThis {
                                        guard.subpage_active = true;
                                        guard.subpage_tab_index = 0;
                                        guard.init_subpage_form();
                                    } else {
                                        // Allow entering About full-page when About virtual entry is selected.
                                        let about_idx = guard.ports.len().saturating_add(2);
                                        if guard.selected == about_idx {
                                            guard.subpage_active = true;
                                            // no form to init; about page reads its own cache
                                        }
                                    }
                                    guard.clear_error();
                                } else {
                                    log::error!("[TUI] failed to lock app");
                                }
                            }
                            Action::MoveNext => {
                                if let Ok(mut guard) = app.lock() {
                                    if guard.subpage_active {
                                        // Log tab navigation else form cursor
                                        if is_log_tab(&guard) {
                                            let total = guard.logs.len();
                                            if total > 0 {
                                                guard.log_selected =
                                                    (guard.log_selected + 1) % total;
                                                let term_h =
                                                    terminal.size().map(|r| r.height).unwrap_or(24);
                                                adjust_log_view(&mut guard, term_h);
                                            }
                                        } else if let Some(form) = guard.subpage_form.as_mut() {
                                            let total = BASE_FIELD_COUNT
                                                .saturating_add(form.registers.len());
                                            if total > 0 {
                                                form.cursor = (form.cursor + 1) % total;
                                            }
                                        } else {
                                            // If About full-page is active, move view down
                                            let about_idx = guard.ports.len().saturating_add(2);
                                            // If About full-page is active, move view down one line
                                            if guard.selected == about_idx {
                                                guard.about_view_offset =
                                                    guard.about_view_offset.saturating_add(1);
                                            }
                                        }
                                    } else {
                                        // Use visual navigation so trailing virtual entries (Refresh / Manual)
                                        // Can be selected in the UI
                                        guard.next_visual();
                                    }
                                    guard.clear_error();
                                } else {
                                    log::error!("[TUI] failed to lock app for MoveNext");
                                }
                            }
                            Action::MovePrev => {
                                if let Ok(mut guard) = app.lock() {
                                    if guard.subpage_active {
                                        if is_log_tab(&guard) {
                                            let total = guard.logs.len();
                                            if total > 0 {
                                                if guard.log_selected == 0 {
                                                    guard.log_selected = total - 1;
                                                } else {
                                                    guard.log_selected -= 1;
                                                }
                                                let term_h =
                                                    terminal.size().map(|r| r.height).unwrap_or(24);
                                                adjust_log_view(&mut guard, term_h);
                                            }
                                        } else if let Some(form) = guard.subpage_form.as_mut() {
                                            let total = BASE_FIELD_COUNT
                                                .saturating_add(form.registers.len());
                                            if total > 0 {
                                                if form.cursor == 0 {
                                                    form.cursor = total - 1;
                                                } else {
                                                    form.cursor -= 1;
                                                }
                                            }
                                        } else {
                                            // If About full-page is active, move view up
                                            let about_idx = guard.ports.len().saturating_add(2);
                                            if guard.selected == about_idx {
                                                guard.about_view_offset =
                                                    guard.about_view_offset.saturating_sub(1);
                                            }
                                        }
                                    } else {
                                        guard.prev_visual();
                                    }
                                    guard.clear_error();
                                } else {
                                    log::error!("[TUI] failed to lock app for MovePrev");
                                }
                            }
                            Action::PageUp => {
                                if let Ok(mut guard) = app.lock() {
                                    if is_log_tab(&guard) {
                                        guard.page_up(LOG_PAGE_JUMP);
                                    } else {
                                        // If About subpage is active, page up
                                        let about_idx = guard.ports.len().saturating_add(2);
                                        if guard.subpage_active && guard.selected == about_idx {
                                            // move up by a page (use LOG_PAGE_JUMP as a reasonable page size)
                                            guard.about_view_offset = guard
                                                .about_view_offset
                                                .saturating_sub(LOG_PAGE_JUMP);
                                        }
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::PageDown => {
                                if let Ok(mut guard) = app.lock() {
                                    if is_log_tab(&guard) {
                                        guard.page_down(LOG_PAGE_JUMP);
                                    } else {
                                        let about_idx = guard.ports.len().saturating_add(2);
                                        if guard.subpage_active && guard.selected == about_idx {
                                            guard.about_view_offset = guard
                                                .about_view_offset
                                                .saturating_add(LOG_PAGE_JUMP);
                                        }
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::JumpTop => {
                                if let Ok(mut guard) = app.lock() {
                                    if is_log_tab(&guard) {
                                        // Jump to top: bottom index becomes the last index of the first page
                                        guard.log_view_offset = 0;
                                        guard.log_auto_scroll = false;
                                    } else {
                                        let about_idx = guard.ports.len().saturating_add(2);
                                        if guard.subpage_active && guard.selected == about_idx {
                                            guard.about_view_offset = 0;
                                        }
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::JumpBottom => {
                                if let Ok(mut guard) = app.lock() {
                                    if is_log_tab(&guard) {
                                        let total = guard.logs.len();
                                        if total > 0 {
                                            guard.log_view_offset = total - 1;
                                        } else {
                                            guard.log_view_offset = 0;
                                        }
                                        guard.log_auto_scroll = true;
                                    } else {
                                        let about_idx = guard.ports.len().saturating_add(2);
                                        if guard.subpage_active && guard.selected == about_idx {
                                            // For about, bottom resets to top to show start
                                            guard.about_view_offset = 0;
                                        }
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::ToggleFollow => {
                                if let Ok(mut guard) = app.lock() {
                                    if is_log_tab(&guard) {
                                        // Toggle following newest logs
                                        guard.log_auto_scroll = !guard.log_auto_scroll;
                                        if guard.log_auto_scroll {
                                            // Move view to bottom (latest)
                                            let total = guard.logs.len();
                                            if total > 0 {
                                                guard.log_view_offset = total - 1;
                                            } else {
                                                guard.log_view_offset = 0;
                                            }
                                        }
                                    }
                                    guard.clear_error();
                                }
                            }
                            // Removed SwitchMode / CycleMode / ShowModeSelector branches after unifying mode
                            Action::EnterSubpage(_) => {
                                if let Ok(mut guard) = app.lock() {
                                    let state = guard
                                        .port_states
                                        .get(guard.selected)
                                        .cloned()
                                        .unwrap_or(crate::protocol::status::PortState::Free);
                                    if state == crate::protocol::status::PortState::OccupiedByThis {
                                        guard.subpage_active = true;
                                        guard.init_subpage_form();
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::AddRegister => {
                                if let Ok(mut guard) = app.lock() {
                                    if guard.subpage_active {
                                        if guard.subpage_form.is_none() {
                                            guard.init_subpage_form();
                                        }
                                        if let Some(form) = guard.subpage_form.as_mut() {
                                            form.registers.push(
                                                crate::protocol::status::RegisterEntry {
                                                    slave_id: 1,
                                                    role: crate::protocol::status::EntryRole::Slave,
                                                    mode: crate::protocol::status::RegisterMode::Coils,
                                                    address: 0,
                                                    length: 1,
                                                    values: vec![0u16; 1],
                                                    req_success: 0,
                                                    req_total: 0,
                                                    next_poll_at: std::time::Instant::now(),
                                                    pending_requests: Vec::new(),
                                                }
                                            );
                                        }
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::DeleteRegister => {
                                if let Ok(mut guard) = app.lock() {
                                    if let Some(form) = guard.subpage_form.as_mut() {
                                        form.registers.pop();
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::EditToggle => {
                                if let Ok(mut guard) = app.lock() {
                                    if let Some(form) = guard.subpage_form.as_mut() {
                                        form.editing = !form.editing;
                                        if form.editing {
                                            match form.cursor {
                                                    0 => form.editing_field = Some(crate::protocol::status::EditingField::Loop),
                                                    // idx 1 is the master_passive toggle and should not enter editing mode
                                                    1 => {
                                                        // Prevent entering edit mode for this cursor position
                                                        form.editing = false;
                                                        form.editing_field = None;
                                                    }
                                                    2 => form.editing_field = Some(crate::protocol::status::EditingField::Baud),
                                                    3 => form.editing_field = Some(crate::protocol::status::EditingField::Parity),
                                                    4 => form.editing_field = Some(crate::protocol::status::EditingField::DataBits),
                                                    5 => form.editing_field = Some(crate::protocol::status::EditingField::StopBits),
                                                    6 => form.editing_field = Some(crate::protocol::status::EditingField::GlobalInterval),
                                                    7 => form.editing_field = Some(crate::protocol::status::EditingField::GlobalTimeout),
                                                    n => {
                                                        let ridx = n.saturating_sub(8);
                                                        form.editing_field = Some(crate::protocol::status::EditingField::RegisterField { idx: ridx, field: crate::protocol::status::RegisterField::SlaveId });
                                                    }
                                            }
                                            form.input_buffer.clear();
                                            if let Some(
                                                crate::protocol::status::EditingField::Baud,
                                            ) = form.editing_field.clone()
                                            {
                                                let presets: [u32; 8] = [
                                                    1200, 2400, 4800, 9600, 19200, 38400, 57600,
                                                    115200,
                                                ];
                                                let _custom_idx = presets.len();
                                                let idx = presets
                                                    .iter()
                                                    .position(|&p| p == form.baud)
                                                    .unwrap_or(_custom_idx);
                                                form.edit_choice_index = Some(idx);
                                                if idx == presets.len() {
                                                    form.input_buffer = form.baud.to_string();
                                                }
                                                form.edit_confirmed = false;
                                            }
                                        } else {
                                            form.editing_field = None;
                                            form.input_buffer.clear();
                                            form.edit_choice_index = None;
                                            form.edit_confirmed = false;
                                        }
                                    }
                                    guard.clear_error();
                                }
                            }
                            Action::ExitSubpage => {
                                if let Ok(mut guard) = app.lock() {
                                    guard.subpage_active = false;
                                    guard.clear_error();
                                }
                            }
                            Action::TogglePort => {
                                if let Ok(mut guard) = app.lock() {
                                    guard.toggle_selected_port();
                                    let state = guard
                                        .port_states
                                        .get(guard.selected)
                                        .cloned()
                                        .unwrap_or(crate::protocol::status::PortState::Free);
                                    if state != crate::protocol::status::PortState::OccupiedByThis {
                                        // Nothing to do: single-pane UI keeps left selection
                                    }
                                } else {
                                    log::error!("[TUI] failed to lock app for TogglePort");
                                }
                            }
                            Action::SwitchNext => {
                                if let Ok(mut guard) = app.lock() {
                                    // Unified mode: SwitchNext now no-op
                                    guard.clear_error();
                                }
                            }
                            Action::SwitchPrev => {
                                if let Ok(mut guard) = app.lock() {
                                    // unified mode: SwitchPrev now no-op
                                    guard.clear_error();
                                }
                            }
                            Action::ClearError => {
                                if let Ok(mut guard) = app.lock() {
                                    guard.clear_error();
                                }
                            }
                            Action::QuickScan => {
                                if let Ok(mut guard) = app.lock() {
                                    // Only meaningful when Refresh action item is selected
                                    if guard.selected >= guard.ports.len()
                                        && guard.selected == guard.ports.len()
                                    {
                                        // refresh item
                                        guard.quick_scan();
                                    }
                                }
                            }
                            Action::None => {}
                        }
                    }
                }
            } // end key event match
        } // end poll
          // No automatic error clearing; errors are cleared manually via the UI
    } // end loop

    terminal.clear()?;
    Ok(())
}
