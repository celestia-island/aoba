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
use crate::protocol::status::{AppMode, ModeOverlayIndex, SubpageTab};
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
    crate::protocol::status::ui::ui_subpage_active_get(app)
        && crate::protocol::status::ui::ui_subpage_tab_index_get(app) == SubpageTab::Log
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
        let _ = crate::protocol::status::status_rw::write_status(&app, |g| {
            crate::protocol::status::ui::ui_error_set(
                g,
                Some((
                    "demo forced error: AOBA_TUI_FORCE_ERROR".to_string(),
                    chrono::Local::now(),
                )),
            );
            Ok(())
        });
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
                            let _ =
                                crate::protocol::status::status_rw::write_status(&app_clone, |g| {
                                    g.refresh();
                                    Ok(())
                                });
                            let _ = core_tx.send(CoreToUi::Refreshed);
                        }
                        UiToCore::Quit => {
                            return;
                        }
                        UiToCore::PausePolling => {
                            let _ =
                                crate::protocol::status::status_rw::write_status(&app_clone, |g| {
                                    g.pause_and_reset_slave_listen();
                                    Ok(())
                                });
                            let _ = core_tx.send(CoreToUi::Refreshed);
                        }
                        UiToCore::ResumePolling => {
                            let _ =
                                crate::protocol::status::status_rw::write_status(&app_clone, |g| {
                                    g.resume_slave_listen();
                                    Ok(())
                                });
                            let _ = core_tx.send(CoreToUi::Refreshed);
                        }
                    }
                }

                // Lightweight port list refresh (every 1s)
                if last_ports_refresh.elapsed() >= Duration::from_millis(1000) {
                    let _ = crate::protocol::status::status_rw::write_status(&app_clone, |g| {
                        g.refresh_ports_only();
                        Ok(())
                    });
                    last_ports_refresh = std::time::Instant::now();
                    let _ = core_tx.send(CoreToUi::Refreshed);
                }

                // Full device scan (includes external commands) at lower frequency (e.g. every 15s) to avoid UI stalls
                if last_full_scan.elapsed() >= Duration::from_secs(15) {
                    let _ = crate::protocol::status::status_rw::write_status(&app_clone, |g| {
                        g.refresh();
                        Ok(())
                    });
                    last_full_scan = std::time::Instant::now();
                    let _ = core_tx.send(CoreToUi::Refreshed);
                }

                // Drive polling + sync runtime configs + drain events (keep lock short)
                let _ = crate::protocol::status::status_rw::write_status(&app_clone, |g| {
                    g.sync_runtime_configs();
                    g.drive_slave_polling();
                    g.drain_runtime_events();
                    g.tick_spinner();
                    Ok(())
                });
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

        // Rendering only (read state) using read_status
        let _ = crate::protocol::status::status_rw::read_status(&app, |g| {
            terminal.draw(|f| crate::tui::ui::render_ui(f, &mut g.clone()))?;
            Ok(())
        });

        // Poll for input
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                if key.kind != crossterm::event::KeyEventKind::Press {
                    continue; // Ignore non-initial key press (repeat / release)
                }
                // Main keyboard event handling entry
                // Mode overlay handling
                let overlay_active = crate::protocol::status::status_rw::read_status(&app, |g| {
                    Ok(crate::protocol::status::ui::ui_mode_overlay_active_get(g))
                })
                .unwrap_or(false);
                if overlay_active {
                    use crossterm::event::KeyCode as KC;
                    // Apply overlay changes via write_status
                    let _ = crate::protocol::status::status_rw::write_status(&app, |guard| {
                        match key.code {
                            KC::Esc => {
                                crate::protocol::status::ui::ui_mode_overlay_active_set(
                                    guard, false,
                                );
                            }
                            KC::Tab => {
                                let cur =
                                    crate::protocol::status::ui::ui_mode_overlay_index_get(guard)
                                        .as_usize();
                                let new = (cur + 1) % 2;
                                crate::protocol::status::ui::ui_mode_overlay_index_set(
                                    guard,
                                    match new {
                                        0 => ModeOverlayIndex::Modbus,
                                        1 => ModeOverlayIndex::Mqtt,
                                        _ => ModeOverlayIndex::Modbus,
                                    },
                                );
                            }
                            KC::Enter => {
                                let sel =
                                    match crate::protocol::status::ui::ui_mode_overlay_index_get(
                                        guard,
                                    ) {
                                        ModeOverlayIndex::Modbus => AppMode::Modbus,
                                        ModeOverlayIndex::Mqtt => AppMode::Mqtt,
                                    };
                                if crate::protocol::status::ui::ui_app_mode_get(guard) != sel {
                                    crate::protocol::status::ui::ui_app_mode_set(guard, sel);
                                    // inline save_current_port_state to avoid direct Status method call
                                    if crate::protocol::status::ui::ui_selected_get(guard)
                                        < guard.ports.list.len()
                                    {
                                        if let Some(info) = guard.ports.list.get(
                                            crate::protocol::status::ui::ui_selected_get(guard),
                                        ) {
                                            let snap = crate::protocol::status::PerPortState {
                                                subpage_active: crate::protocol::status::ui::ui_subpage_active_get(guard),
                                                subpage_form: crate::protocol::status::ui::ui_subpage_form_get(guard),
                                                subpage_tab_index: crate::protocol::status::ui::ui_subpage_tab_index_get(guard),
                                                logs: crate::protocol::status::ui::ui_logs_get(guard),
                                                log_selected: crate::protocol::status::ui::ui_log_selected_get(guard),
                                                log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(guard),
                                                log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(guard),
                                                input_mode: crate::protocol::status::ui::ui_input_mode_get(guard),
                                                input_editing: crate::protocol::status::ui::ui_input_editing_get(guard),
                                                input_buffer: crate::protocol::status::ui::ui_input_buffer_get(guard),
                                                app_mode: crate::protocol::status::ui::ui_app_mode_get(guard),
                                                page: crate::protocol::status::ui::ui_pages_last_get(guard),
                                            };
                                            guard
                                                .per_port
                                                .states
                                                .insert(info.port_name.clone(), snap);
                                        }
                                    }
                                }
                                crate::protocol::status::ui::ui_mode_overlay_active_set(
                                    guard, false,
                                );
                            }
                            _ => {}
                        }
                        crate::protocol::status::ui::ui_error_set(guard, None);
                        Ok(())
                    });
                    // Redraw immediately (snapshot)
                    let _ = crate::protocol::status::status_rw::read_status(&app, |g| {
                        terminal.draw(|f| crate::tui::ui::render_ui(f, &mut g.clone()))?;
                        Ok(())
                    });
                    continue;
                } else {
                    // nothing to drop
                }

                // Re-evaluate editing after potential selector handling
                let is_editing = crate::protocol::status::status_rw::read_status(&app, |g| {
                    Ok(crate::protocol::status::ui::ui_subpage_form_get(g)
                        .as_ref()
                        .map(|f| f.editing)
                        .unwrap_or(false))
                })
                .unwrap_or(false);

                if is_editing {
                    use crossterm::event::KeyCode as KC;
                    let _ = crate::protocol::status::status_rw::write_status(&app, |guard| {
                        let mut pending_error: Option<String> = None;
                        if let Some(form) = guard.ui.subpage_form.as_mut() {
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
                                            return Ok(());
                                        }
                                    }
                                    let mut commit_success = true;
                                    if let Some(field) = &form.editing_field {
                                        match field {
                                            crate::protocol::status::EditingField::Baud => {
                                                let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                                                if let Some(idx) = form.edit_choice_index {
                                                    if idx < presets.len() {
                                                        form.baud = presets[idx];
                                                    } else {
                                                        // Custom: must parse and validate [1200..=2_000_000]
                                                        if !form.input_buffer.is_empty() {
                                                            if let Ok(v) = form.input_buffer.parse::<u32>() {
                                                                if (1200..=2_000_000).contains(&v) {
                                                                    form.baud = v;
                                                                } else {
                                                                    pending_error = Some(lang().protocol.modbus.invalid_baud_range.clone());
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
                                                        if let Ok(v) = form.input_buffer.parse::<u32>() {
                                                            if (1200..=2_000_000).contains(&v) {
                                                                form.baud = v;
                                                            } else {
                                                                pending_error = Some(lang().protocol.modbus.invalid_baud_range.clone());
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
                            crate::protocol::status::ui::ui_error_set(
                                guard,
                                Some((msg.into(), chrono::Local::now())),
                            );
                        } else {
                            crate::protocol::status::ui::ui_error_set(guard, None);
                        }
                        Ok(())
                    });
                    // Redraw immediately snapshot
                    let _ = crate::protocol::status::status_rw::read_status(&app, |g| {
                        terminal.draw(|f| crate::tui::ui::render_ui(f, &mut g.clone()))?;
                        Ok(())
                    });
                    continue;
                }

                // Input / log handling via status_rw helpers to avoid direct lock usage
                let _ = crate::protocol::status::status_rw::write_status(&app, |guard| {
                    if is_log_tab(&guard) {
                        use crossterm::event::KeyCode as KC;
                        if crate::protocol::status::ui::ui_input_editing_get(guard) {
                            match key.code {
                                KC::Char(c) => {
                                    if crate::protocol::status::ui::ui_input_mode_get(guard)
                                        == InputMode::Ascii
                                    {
                                        let mut buf =
                                            crate::protocol::status::ui::ui_input_buffer_get(guard);
                                        buf.push(c);
                                        crate::protocol::status::ui::ui_input_buffer_set(
                                            guard, buf,
                                        );
                                    } else if c.is_ascii_hexdigit() || c.is_whitespace() {
                                        let mut buf =
                                            crate::protocol::status::ui::ui_input_buffer_get(guard);
                                        buf.push(c);
                                        crate::protocol::status::ui::ui_input_buffer_set(
                                            guard, buf,
                                        );
                                    }
                                }
                                KC::Backspace => {
                                    let mut buf =
                                        crate::protocol::status::ui::ui_input_buffer_get(guard);
                                    buf.pop();
                                    crate::protocol::status::ui::ui_input_buffer_set(guard, buf);
                                }
                                KC::Enter => {
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
                                        raw: crate::protocol::status::ui::ui_input_buffer_get(
                                            guard,
                                        ),
                                        parsed: Some(parsed),
                                    };
                                    // inline Status::append_log (migrated to accessors)
                                    const MAX: usize = 1000;
                                    let mut logs = crate::protocol::status::ui::ui_logs_get(guard);
                                    logs.push(entry);
                                    if logs.len() > MAX {
                                        let excess = logs.len() - MAX;
                                        logs.drain(0..excess);
                                        let log_sel =
                                            crate::protocol::status::ui::ui_log_selected_get(guard);
                                        if log_sel >= logs.len() {
                                            crate::protocol::status::ui::ui_log_selected_set(
                                                guard,
                                                logs.len().saturating_sub(1),
                                            );
                                        }
                                    }
                                    crate::protocol::status::ui::ui_logs_set(guard, logs);
                                    if crate::protocol::status::ui::ui_log_auto_scroll_get(guard) {
                                        let logs2 = crate::protocol::status::ui::ui_logs_get(guard);
                                        if logs2.is_empty() {
                                            crate::protocol::status::ui::ui_log_view_offset_set(
                                                guard, 0,
                                            );
                                        } else {
                                            let last = logs2.len().saturating_sub(1);
                                            crate::protocol::status::ui::ui_log_view_offset_set(
                                                guard, last,
                                            );
                                            crate::protocol::status::ui::ui_log_selected_set(
                                                guard, last,
                                            );
                                        }
                                    }
                                    crate::protocol::status::ui::ui_input_buffer_set(
                                        guard,
                                        String::new(),
                                    );
                                    crate::protocol::status::ui::ui_input_editing_set(guard, false);
                                    return Ok(());
                                }
                                _ => {}
                            }
                        } else {
                            // Not editing: allow quick toggles for edit / mode
                            match key.code {
                                KC::Enter | KC::Char('i') => {
                                    crate::protocol::status::ui::ui_input_editing_set(guard, true);
                                    crate::protocol::status::ui::ui_error_set(guard, None);
                                    return Ok(());
                                }
                                KC::Char('m') => {
                                    crate::protocol::status::ui::ui_input_mode_set(
                                        guard,
                                        match crate::protocol::status::ui::ui_input_mode_get(guard)
                                        {
                                            InputMode::Ascii => InputMode::Hex,
                                            InputMode::Hex => InputMode::Ascii,
                                        },
                                    );
                                    crate::protocol::status::ui::ui_error_set(guard, None);
                                    return Ok(());
                                }
                                KC::Up | KC::Char('k') => {
                                    let total =
                                        crate::protocol::status::ui::ui_logs_get(guard).len();
                                    if total > 0 {
                                        let mut sel =
                                            crate::protocol::status::ui::ui_log_selected_get(guard);
                                        if sel == 0 {
                                            sel = total.saturating_sub(1);
                                        } else {
                                            sel = sel.saturating_sub(1);
                                        }
                                        crate::protocol::status::ui::ui_log_selected_set(
                                            guard, sel,
                                        );
                                        let term_h =
                                            terminal.size().map(|r| r.height).unwrap_or(24);
                                        if !crate::protocol::status::ui::ui_logs_get(guard)
                                            .is_empty()
                                        {
                                            let bottom_len =
                                                if crate::protocol::status::ui::ui_error_get(guard)
                                                    .is_some()
                                                    || guard.ui.subpage_active
                                                {
                                                    2
                                                } else {
                                                    1
                                                };
                                            let logs_area_h =
                                                (term_h as usize).saturating_sub(bottom_len + 5);
                                            let inner_h = logs_area_h.saturating_sub(2);
                                            let groups_per_screen = std::cmp::max(
                                                1usize,
                                                inner_h / crate::protocol::status::LOG_GROUP_HEIGHT,
                                            );
                                            let bottom = if crate::protocol::status::ui::ui_log_auto_scroll_get(guard) {
                                                crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1)
                                            } else {
                                                std::cmp::min(
                                                    crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                    crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1),
                                                )
                                            };
                                            let top =
                                                (bottom + 1).saturating_sub(groups_per_screen);
                                            if crate::protocol::status::ui::ui_log_selected_get(guard) < top {
                                                crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                let half = groups_per_screen / 2;
                                                let new_bottom = std::cmp::min(
                                                    crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1),
                                                    crate::protocol::status::ui::ui_log_selected_get(guard) + half,
                                                );
                                                crate::protocol::status::ui::ui_log_view_offset_set(guard, new_bottom);
                                            } else if crate::protocol::status::ui::ui_log_selected_get(guard) > bottom {
                                                crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                crate::protocol::status::ui::ui_log_view_offset_set(guard, crate::protocol::status::ui::ui_log_selected_get(guard));
                                            }
                                        }
                                    }
                                    crate::protocol::status::ui::ui_error_set(guard, None);
                                    return Ok(());
                                }
                                KC::Char('c') => {
                                    if !crate::protocol::status::ui::ui_log_clear_pending_get(guard)
                                    {
                                        crate::protocol::status::ui::ui_log_clear_pending_set(
                                            guard, true,
                                        );
                                    } else {
                                        crate::protocol::status::ui::ui_logs_set(guard, Vec::new());
                                        crate::protocol::status::ui::ui_log_selected_set(guard, 0);
                                        crate::protocol::status::ui::ui_log_view_offset_set(
                                            guard, 0,
                                        );
                                        crate::protocol::status::ui::ui_log_auto_scroll_set(
                                            guard, true,
                                        );
                                        crate::protocol::status::ui::ui_log_clear_pending_set(
                                            guard, false,
                                        );
                                        // inline save_current_port_state
                                        if guard.ui.selected < guard.ports.list.len() {
                                            if let Some(info) =
                                                guard.ports.list.get(guard.ui.selected)
                                            {
                                                let snap = crate::protocol::status::PerPortState {
                                                    subpage_active: crate::protocol::status::ui::ui_subpage_active_get(guard),
                                                    subpage_form: crate::protocol::status::ui::ui_subpage_form_get(guard),
                                                    subpage_tab_index: crate::protocol::status::ui::ui_subpage_tab_index_get(guard),
                                                    logs: crate::protocol::status::ui::ui_logs_get(guard),
                                                    log_selected: crate::protocol::status::ui::ui_log_selected_get(guard),
                                                    log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                    log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(guard),
                                                    log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(guard),
                                                    input_mode: crate::protocol::status::ui::ui_input_mode_get(guard),
                                                    input_editing: crate::protocol::status::ui::ui_input_editing_get(guard),
                                                    input_buffer: crate::protocol::status::ui::ui_input_buffer_get(guard),
                                                    app_mode: crate::protocol::status::ui::ui_app_mode_get(guard),
                                                    page: crate::protocol::status::ui::ui_pages_last_get(guard),
                                                };
                                                guard
                                                    .per_port
                                                    .states
                                                    .insert(info.port_name.clone(), snap);
                                            }
                                        }
                                    }
                                    crate::protocol::status::ui::ui_error_set(guard, None);
                                    return Ok(());
                                }
                                KC::Down | KC::Char('j') => {
                                    let total =
                                        crate::protocol::status::ui::ui_logs_get(guard).len();
                                    if total > 0 {
                                        let sel =
                                            crate::protocol::status::ui::ui_log_selected_get(guard);
                                        let sel = (sel + 1) % total;
                                        crate::protocol::status::ui::ui_log_selected_set(
                                            guard, sel,
                                        );
                                        let term_h =
                                            terminal.size().map(|r| r.height).unwrap_or(24);
                                        if !crate::protocol::status::ui::ui_logs_get(guard)
                                            .is_empty()
                                        {
                                            let bottom_len = if crate::protocol::status::ui::ui_error_get(guard).is_some()
                                                || crate::protocol::status::ui::ui_subpage_active_get(guard)
                                            {
                                                2
                                            } else {
                                                1
                                            };
                                            let logs_area_h =
                                                (term_h as usize).saturating_sub(bottom_len + 5);
                                            let inner_h = logs_area_h.saturating_sub(2);
                                            let groups_per_screen = std::cmp::max(
                                                1usize,
                                                inner_h / crate::protocol::status::LOG_GROUP_HEIGHT,
                                            );
                                            let bottom = if crate::protocol::status::ui::ui_log_auto_scroll_get(guard) {
                                                            crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1)
                                                        } else {
                                                            std::cmp::min(
                                                                crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                                crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1),
                                                            )
                                                        };
                                            let top =
                                                (bottom + 1).saturating_sub(groups_per_screen);
                                            if crate::protocol::status::ui::ui_log_selected_get(guard) < top {
                                                            crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                            let half = groups_per_screen / 2;
                                                            let new_bottom = std::cmp::min(
                                                                crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1),
                                                                crate::protocol::status::ui::ui_log_selected_get(guard) + half,
                                                            );
                                                            crate::protocol::status::ui::ui_log_view_offset_set(guard, new_bottom);
                                                        } else if crate::protocol::status::ui::ui_log_selected_get(guard) > bottom {
                                                            crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                            crate::protocol::status::ui::ui_log_view_offset_set(guard, crate::protocol::status::ui::ui_log_selected_get(guard));
                                                        }
                                        }
                                    }
                                    crate::protocol::status::ui::ui_error_set(guard, None);
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                    } else {
                        // Global mode cycle (m) when not in log tab quick-toggle context
                        use crossterm::event::KeyCode as KC;
                        if key.code == KC::Char('m') {
                            crate::protocol::status::ui::ui_mode_overlay_active_set(guard, true);
                            crate::protocol::status::ui::ui_mode_overlay_index_set(
                                guard,
                                match crate::protocol::status::ui::ui_app_mode_get(guard) {
                                    AppMode::Modbus => ModeOverlayIndex::Modbus,
                                    AppMode::Mqtt => ModeOverlayIndex::Mqtt,
                                },
                            );
                            crate::protocol::status::ui::ui_error_set(guard, None);
                            return Ok(());
                        }
                    }
                    Ok(())
                });

                // Subpage first chance (use write_status helper)
                let handled = crate::protocol::status::status_rw::write_status(&app, |guard| {
                    let handled = crate::tui::ui::pages::handle_key_in_subpage(key, guard, &bus);
                    if handled {
                        crate::protocol::status::ui::ui_error_set(guard, None);
                    }
                    Ok(handled)
                })
                .unwrap_or(false);
                if handled {
                    continue;
                }

                {
                    // Try page-level mapping first (inner pages can override), fall back to global mapping
                    let mut action_opt =
                        crate::protocol::status::status_rw::read_status(&app, |guard| {
                            Ok(crate::tui::ui::pages::map_key_in_page(key, guard))
                        })
                        .unwrap_or(None);
                    if action_opt.is_none() {
                        match map_key(key.code) {
                            Action::None => {}
                            a => action_opt = Some(a),
                        }
                    }

                    if let Some(action) = action_opt {
                        match action {
                            Action::Quit => {
                                let allowed =
                                    crate::protocol::status::status_rw::read_status(&app, |g| {
                                        Ok(!g.ui.subpage_active
                                            && !g
                                                .ui
                                                .subpage_form
                                                .as_ref()
                                                .map(|f| f.editing)
                                                .unwrap_or(false))
                                    })
                                    .unwrap_or(false);
                                if allowed {
                                    let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Quit);
                                    break;
                                }
                            }
                            Action::LeavePage => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if guard.ui.subpage_active {
                                            guard.ui.subpage_active = false;
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::EnterPage => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        let state = guard
                                            .ports
                                            .states
                                            .get(crate::protocol::status::ui::ui_selected_get(
                                                guard,
                                            ))
                                            .cloned()
                                            .unwrap_or(crate::protocol::status::PortState::Free);
                                        if state
                                            == crate::protocol::status::PortState::OccupiedByThis
                                        {
                                            crate::protocol::status::ui::ui_subpage_active_set(
                                                guard, true,
                                            );
                                            crate::protocol::status::ui::ui_subpage_tab_index_set(
                                                guard,
                                                SubpageTab::Config,
                                            );
                                            guard.init_subpage_form();
                                        } else {
                                            let about_idx =
                                                guard.ports.list.len().saturating_add(2);
                                            if guard.ui.selected == about_idx {
                                                crate::protocol::status::ui::ui_subpage_active_set(
                                                    guard, true,
                                                );
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::MoveNext => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if guard.ui.subpage_active {
                                            if is_log_tab(&guard) {
                                                let total =
                                                    crate::protocol::status::ui::ui_logs_get(guard)
                                                        .len();
                                                if total > 0 {
                                                    let sel = (crate::protocol::status::ui::ui_log_selected_get(guard) + 1) % total;
                                                    crate::protocol::status::ui::ui_log_selected_set(guard, sel);
                                                    let term_h = terminal
                                                        .size()
                                                        .map(|r| r.height)
                                                        .unwrap_or(24);
                                                    if !crate::protocol::status::ui::ui_logs_get(
                                                        guard,
                                                    )
                                                    .is_empty()
                                                    {
                                                        let bottom_len = if crate::protocol::status::ui::ui_error_get(guard).is_some()
                                                            || guard.ui.subpage_active
                                                        {
                                                            2
                                                        } else {
                                                            1
                                                        };
                                                        let logs_area_h = (term_h as usize)
                                                            .saturating_sub(bottom_len + 5);
                                                        let inner_h = logs_area_h.saturating_sub(2);
                                                        let groups_per_screen = std::cmp::max(1usize, inner_h / crate::protocol::status::LOG_GROUP_HEIGHT);
                                                        let bottom = if crate::protocol::status::ui::ui_log_auto_scroll_get(guard) {
                                                            crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1)
                                                        } else {
                                                            std::cmp::min(
                                                                crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                                crate::protocol::status::ui::ui_logs_get(guard)
                                                                    .len()
                                                                    .saturating_sub(1),
                                                            )
                                                        };
                                                        let top = (bottom + 1)
                                                            .saturating_sub(groups_per_screen);
                                                        if crate::protocol::status::ui::ui_log_selected_get(guard) < top {
                                                            crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                            let half = groups_per_screen / 2;
                                                            let new_bottom = std::cmp::min(
                                                                crate::protocol::status::ui::ui_logs_get(guard)
                                                                    .len()
                                                                    .saturating_sub(1),
                                                                crate::protocol::status::ui::ui_log_selected_get(guard) + half,
                                                            );
                                                            crate::protocol::status::ui::ui_log_view_offset_set(guard, new_bottom);
                                                        } else if crate::protocol::status::ui::ui_log_selected_get(guard) > bottom {
                                                            crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                            crate::protocol::status::ui::ui_log_view_offset_set(guard, crate::protocol::status::ui::ui_log_selected_get(guard));
                                                        }
                                                    }
                                                }
                                            } else if let Some(form) =
                                                guard.ui.subpage_form.as_mut()
                                            {
                                                let total = BASE_FIELD_COUNT
                                                    .saturating_add(form.registers.len());
                                                if total > 0 {
                                                    form.cursor = (form.cursor + 1) % total;
                                                }
                                            } else {
                                                let about_idx =
                                                    guard.ports.list.len().saturating_add(2);
                                                if crate::protocol::status::ui::ui_selected_get(
                                                    guard,
                                                ) == about_idx
                                                {
                                                    guard.ports.about_view_offset = guard
                                                        .ports
                                                        .about_view_offset
                                                        .saturating_add(1);
                                                }
                                            }
                                        } else {
                                            // inline Status::next_visual
                                            {
                                                // ports + Refresh + Manual + About = ports + 3 virtual entries
                                                let total =
                                                    guard.ports.list.len().saturating_add(3);
                                                if total != 0 {
                                                    let was_real =
                                                        crate::protocol::status::ui::ui_selected_get(
                                                            guard,
                                                        )
                                                            < guard.ports.list.len();
                                                    if was_real {
                                                        // inline save_current_port_state
                                                        if crate::protocol::status::ui::ui_selected_get(guard) < guard.ports.list.len() {
                                                            if let Some(info) = guard.ports.list.get(crate::protocol::status::ui::ui_selected_get(guard)) {
                                                                let snap = crate::protocol::status::PerPortState {
                                                                    subpage_active: crate::protocol::status::ui::ui_subpage_active_get(guard),
                                                                    subpage_form: crate::protocol::status::ui::ui_subpage_form_get(guard),
                                                                    subpage_tab_index: crate::protocol::status::ui::ui_subpage_tab_index_get(guard),
                                                                    logs: crate::protocol::status::ui::ui_logs_get(guard),
                                                                    log_selected: crate::protocol::status::ui::ui_log_selected_get(guard),
                                                                    log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                                    log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(guard),
                                                                    log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(guard),
                                                                    input_mode: crate::protocol::status::ui::ui_input_mode_get(guard),
                                                                    input_editing: crate::protocol::status::ui::ui_input_editing_get(guard),
                                                                    input_buffer: crate::protocol::status::ui::ui_input_buffer_get(guard),
                                                                    app_mode: guard.ui.app_mode,
                                                                    page: crate::protocol::status::ui::ui_pages_last_get(guard),
                                                                };
                                                                guard.per_port.states.insert(
                                                                    info.port_name.clone(),
                                                                    snap,
                                                                );
                                                            }
                                                        }
                                                    }
                                                    crate::protocol::status::ui::ui_selected_set(
                                                        guard,
                                                        (crate::protocol::status::ui::ui_selected_get(guard) + 1) % total,
                                                    );
                                                    if crate::protocol::status::ui::ui_selected_get(
                                                        guard,
                                                    ) < guard.ports.list.len()
                                                    {
                                                        // inline load_current_port_state
                                                        if let Some(info) = guard.ports.list.get(crate::protocol::status::ui::ui_selected_get(guard)) {
                                                            if let Some(snap) = guard
                                                                .per_port
                                                                .states
                                                                .get(&info.port_name)
                                                                .cloned()
                                                            {
                                                                if let Some(page) = snap.page {
                                                                    if guard.ui.pages.is_empty() {
                                                                        guard.ui.pages.push(page);
                                                                    } else {
                                                                        *guard
                                                                            .ui
                                                                            .pages
                                                                            .last_mut()
                                                                            .unwrap() = page;
                                                                    }
                                                                    match crate::protocol::status::ui::ui_pages_last_get(guard).unwrap_or_default() {
                                                                        crate::protocol::status::Page::Entry { selected, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            crate::protocol::status::ui::ui_selected_set(guard, selected);
                                                                            crate::protocol::status::ui::ui_input_mode_set(guard, input_mode);
                                                                            crate::protocol::status::ui::ui_input_editing_set(guard, input_editing);
                                                                            crate::protocol::status::ui::ui_input_buffer_set(guard, input_buffer);
                                                                            crate::protocol::status::ui::ui_app_mode_set(guard, app_mode);
                                                                            crate::protocol::status::ui::ui_subpage_active_set(guard, false);
                                                                            crate::protocol::status::ui::ui_subpage_form_set(guard, None);
                                                                        }
                                                                        crate::protocol::status::Page::Modbus { selected, subpage_active, subpage_form, subpage_tab_index, logs, log_selected, log_view_offset, log_auto_scroll, log_clear_pending, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            crate::protocol::status::ui::ui_selected_set(guard, selected);
                                                                            crate::protocol::status::ui::ui_subpage_active_set(guard, subpage_active);
                                                                            crate::protocol::status::ui::ui_subpage_form_set(guard, subpage_form);
                                                                            crate::protocol::status::ui::ui_subpage_tab_index_set(guard, subpage_tab_index);
                                                                            crate::protocol::status::ui::ui_logs_set(guard, logs);
                                                                            crate::protocol::status::ui::ui_log_selected_set(guard, log_selected);
                                                                            crate::protocol::status::ui::ui_log_view_offset_set(guard, log_view_offset);
                                                                            crate::protocol::status::ui::ui_log_auto_scroll_set(guard, log_auto_scroll);
                                                                            crate::protocol::status::ui::ui_log_clear_pending_set(guard, log_clear_pending);
                                                                            crate::protocol::status::ui::ui_input_mode_set(guard, input_mode);
                                                                            crate::protocol::status::ui::ui_input_editing_set(guard, input_editing);
                                                                            crate::protocol::status::ui::ui_input_buffer_set(guard, input_buffer);
                                                                            crate::protocol::status::ui::ui_app_mode_set(guard, app_mode);
                                                                        }
                                                                    }
                                                                } else {
                                                                    guard.ui.subpage_active =
                                                                        snap.subpage_active;
                                                                    guard.ui.subpage_form =
                                                                        snap.subpage_form;
                                                                    guard.ui.subpage_tab_index =
                                                                        snap.subpage_tab_index;
                                                                    crate::protocol::status::ui::ui_logs_set(guard, snap.logs);
                                                                    crate::protocol::status::ui::ui_log_selected_set(guard, snap.log_selected);
                                                                    crate::protocol::status::ui::ui_log_view_offset_set(guard, snap.log_view_offset);
                                                                    crate::protocol::status::ui::ui_log_auto_scroll_set(guard, snap.log_auto_scroll);
                                                                    crate::protocol::status::ui::ui_log_clear_pending_set(guard, snap.log_clear_pending);
                                                                    guard.ui.input_mode =
                                                                        snap.input_mode;
                                                                    guard.ui.input_editing =
                                                                        snap.input_editing;
                                                                    guard.ui.input_buffer =
                                                                        snap.input_buffer;
                                                                    guard.ui.app_mode =
                                                                        snap.app_mode;
                                                                    if guard.ui.pages.is_empty() {
                                                                        guard.ui.pages.push(crate::protocol::status::Page::default());
                                                                    }
                                                                    match guard.ui.pages.last_mut().unwrap() {
                                                                        crate::protocol::status::Page::Entry { selected, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            *selected = guard.ui.selected;
                                                                            *input_mode = guard.ui.input_mode;
                                                                            *input_editing = guard.ui.input_editing;
                                                                            *input_buffer = guard.ui.input_buffer.clone();
                                                                            *app_mode = guard.ui.app_mode;
                                                                        }
                                                                        crate::protocol::status::Page::Modbus { selected, subpage_active, subpage_form, subpage_tab_index, logs, log_selected, log_view_offset, log_auto_scroll, log_clear_pending, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            *selected = guard.ui.selected;
                                                                            *subpage_active = guard.ui.subpage_active;
                                                                            *subpage_form = guard.ui.subpage_form.clone();
                                                                            *subpage_tab_index = guard.ui.subpage_tab_index;
                                                                            *logs = guard.ui.logs.clone();
                                                                            *log_selected = guard.ui.log_selected;
                                                                            *log_view_offset = guard.ui.log_view_offset;
                                                                            *log_auto_scroll = guard.ui.log_auto_scroll;
                                                                            *log_clear_pending = guard.ui.log_clear_pending;
                                                                            *input_mode = guard.ui.input_mode;
                                                                            *input_editing = guard.ui.input_editing;
                                                                            *input_buffer = guard.ui.input_buffer.clone();
                                                                            *app_mode = guard.ui.app_mode;
                                                                        }
                                                                    }
                                                                }
                                                            } else {
                                                                guard.ui.subpage_active = false;
                                                                guard.ui.subpage_form = None;
                                                                guard.ui.subpage_tab_index = crate::protocol::status::SubpageTab::Config;
                                                                crate::protocol::status::ui::ui_logs_set(guard, Vec::new());
                                                                crate::protocol::status::ui::ui_log_selected_set(guard, 0);
                                                                crate::protocol::status::ui::ui_log_view_offset_set(guard, 0);
                                                                crate::protocol::status::ui::ui_log_auto_scroll_set(guard, true);
                                                                guard.ui.input_mode = crate::protocol::status::InputMode::Ascii;
                                                                guard.ui.input_editing = false;
                                                                guard.ui.input_buffer.clear();
                                                                guard.ui.app_mode = crate::protocol::status::AppMode::Modbus;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::MovePrev => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if guard.ui.subpage_active {
                                            if is_log_tab(&guard) {
                                                let total =
                                                    crate::protocol::status::ui::ui_logs_get(guard)
                                                        .len();
                                                if total > 0 {
                                                    let mut sel = crate::protocol::status::ui::ui_log_selected_get(guard);
                                                    if sel == 0 {
                                                        sel = total.saturating_sub(1);
                                                    } else {
                                                        sel = sel.saturating_sub(1);
                                                    }
                                                    crate::protocol::status::ui::ui_log_selected_set(guard, sel);
                                                    let term_h = terminal
                                                        .size()
                                                        .map(|r| r.height)
                                                        .unwrap_or(24);
                                                    if !crate::protocol::status::ui::ui_logs_get(
                                                        guard,
                                                    )
                                                    .is_empty()
                                                    {
                                                        let bottom_len = if crate::protocol::status::ui::ui_error_get(guard)
                                                            .is_some()
                                                            || crate::protocol::status::ui::ui_subpage_active_get(guard)
                                                        {
                                                            2
                                                        } else {
                                                            1
                                                        };
                                                        let logs_area_h = (term_h as usize)
                                                            .saturating_sub(bottom_len + 5);
                                                        let inner_h = logs_area_h.saturating_sub(2);
                                                        let groups_per_screen = std::cmp::max(1usize, inner_h / crate::protocol::status::LOG_GROUP_HEIGHT);
                                                        let bottom = if crate::protocol::status::ui::ui_log_auto_scroll_get(guard) {
                                                                crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1)
                                                            } else {
                                                                std::cmp::min(
                                                                    crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                                    crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1),
                                                                )
                                                            };
                                                        let top = (bottom + 1)
                                                            .saturating_sub(groups_per_screen);
                                                        if crate::protocol::status::ui::ui_log_selected_get(guard) < top {
                                                                crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                                let half = groups_per_screen / 2;
                                                                let new_bottom = std::cmp::min(
                                                                    crate::protocol::status::ui::ui_logs_get(guard).len().saturating_sub(1),
                                                                    crate::protocol::status::ui::ui_log_selected_get(guard) + half,
                                                                );
                                                                crate::protocol::status::ui::ui_log_view_offset_set(guard, new_bottom);
                                                            } else if crate::protocol::status::ui::ui_log_selected_get(guard) > bottom {
                                                                crate::protocol::status::ui::ui_log_auto_scroll_set(guard, false);
                                                                crate::protocol::status::ui::ui_log_view_offset_set(guard, crate::protocol::status::ui::ui_log_selected_get(guard));
                                                            }
                                                    }
                                                }
                                            } else if let Some(form) =
                                                guard.ui.subpage_form.as_mut()
                                            {
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
                                                let about_idx =
                                                    guard.ports.list.len().saturating_add(2);
                                                if guard.ui.selected == about_idx {
                                                    guard.ports.about_view_offset = guard
                                                        .ports
                                                        .about_view_offset
                                                        .saturating_sub(1);
                                                }
                                            }
                                        } else {
                                            // inline Status::prev_visual
                                            {
                                                let total =
                                                    guard.ports.list.len().saturating_add(3);
                                                if total != 0 {
                                                    let was_real =
                                                        guard.ui.selected < guard.ports.list.len();
                                                    if was_real {
                                                        // inline save_current_port_state
                                                        if guard.ui.selected
                                                            < guard.ports.list.len()
                                                        {
                                                            if let Some(info) = guard
                                                                .ports
                                                                .list
                                                                .get(guard.ui.selected)
                                                            {
                                                                let snap = crate::protocol::status::PerPortState {
                                                                    subpage_active: guard.ui.subpage_active,
                                                                    subpage_form: guard.ui.subpage_form.clone(),
                                                                    subpage_tab_index: guard.ui.subpage_tab_index,
                                                                    logs: crate::protocol::status::ui::ui_logs_get(guard),
                                                                    log_selected: crate::protocol::status::ui::ui_log_selected_get(guard),
                                                                    log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(guard),
                                                                    log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(guard),
                                                                    log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(guard),
                                                                    input_mode: crate::protocol::status::ui::ui_input_mode_get(guard),
                                                                    input_editing: crate::protocol::status::ui::ui_input_editing_get(guard),
                                                                    input_buffer: crate::protocol::status::ui::ui_input_buffer_get(guard),
                                                                    app_mode: crate::protocol::status::ui::ui_app_mode_get(guard),
                                                                    page: crate::protocol::status::ui::ui_pages_last_get(guard),
                                                                };
                                                                guard.per_port.states.insert(
                                                                    info.port_name.clone(),
                                                                    snap,
                                                                );
                                                            }
                                                        }
                                                    }
                                                    let cur = crate::protocol::status::ui::ui_selected_get(guard);
                                                    if cur == 0 {
                                                        crate::protocol::status::ui::ui_selected_set(guard, total - 1);
                                                    } else {
                                                        crate::protocol::status::ui::ui_selected_set(guard, cur - 1);
                                                    }
                                                    if guard.ui.selected < guard.ports.list.len() {
                                                        // inline load_current_port_state (same as next_visual)
                                                        if let Some(info) =
                                                            guard.ports.list.get(guard.ui.selected)
                                                        {
                                                            if let Some(snap) = guard
                                                                .per_port
                                                                .states
                                                                .get(&info.port_name)
                                                                .cloned()
                                                            {
                                                                if let Some(page) = snap.page {
                                                                    if guard.ui.pages.is_empty() {
                                                                        guard.ui.pages.push(page);
                                                                    } else {
                                                                        *guard
                                                                            .ui
                                                                            .pages
                                                                            .last_mut()
                                                                            .unwrap() = page;
                                                                    }
                                                                    match guard.ui.pages.last().cloned().unwrap_or_default() {
                                                                        crate::protocol::status::Page::Entry { selected, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            guard.ui.selected = selected;
                                                                            guard.ui.input_mode = input_mode;
                                                                            guard.ui.input_editing = input_editing;
                                                                            guard.ui.input_buffer = input_buffer;
                                                                            guard.ui.app_mode = app_mode;
                                                                            guard.ui.subpage_active = false;
                                                                            guard.ui.subpage_form = None;
                                                                        }
                                                                        crate::protocol::status::Page::Modbus { selected, subpage_active, subpage_form, subpage_tab_index, logs, log_selected, log_view_offset, log_auto_scroll, log_clear_pending, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            guard.ui.selected = selected;
                                                                            guard.ui.subpage_active = subpage_active;
                                                                            guard.ui.subpage_form = subpage_form;
                                                                            guard.ui.subpage_tab_index = subpage_tab_index;
                                                                            crate::protocol::status::ui::ui_logs_set(guard, logs);
                                                                            crate::protocol::status::ui::ui_log_selected_set(guard, log_selected);
                                                                            crate::protocol::status::ui::ui_log_view_offset_set(guard, log_view_offset);
                                                                            crate::protocol::status::ui::ui_log_auto_scroll_set(guard, log_auto_scroll);
                                                                            crate::protocol::status::ui::ui_log_clear_pending_set(guard, log_clear_pending);
                                                                            guard.ui.input_mode = input_mode;
                                                                            guard.ui.input_editing = input_editing;
                                                                            guard.ui.input_buffer = input_buffer;
                                                                            guard.ui.app_mode = app_mode;
                                                                        }
                                                                    }
                                                                } else {
                                                                    guard.ui.subpage_active =
                                                                        snap.subpage_active;
                                                                    guard.ui.subpage_form =
                                                                        snap.subpage_form;
                                                                    guard.ui.subpage_tab_index =
                                                                        snap.subpage_tab_index;
                                                                    crate::protocol::status::ui::ui_logs_set(guard, snap.logs);
                                                                    crate::protocol::status::ui::ui_log_selected_set(guard, snap.log_selected);
                                                                    crate::protocol::status::ui::ui_log_view_offset_set(guard, snap.log_view_offset);
                                                                    crate::protocol::status::ui::ui_log_auto_scroll_set(guard, snap.log_auto_scroll);
                                                                    crate::protocol::status::ui::ui_log_clear_pending_set(guard, snap.log_clear_pending);
                                                                    guard.ui.input_mode =
                                                                        snap.input_mode;
                                                                    guard.ui.input_editing =
                                                                        snap.input_editing;
                                                                    guard.ui.input_buffer =
                                                                        snap.input_buffer;
                                                                    guard.ui.app_mode =
                                                                        snap.app_mode;
                                                                    if guard.ui.pages.is_empty() {
                                                                        guard.ui.pages.push(crate::protocol::status::Page::default());
                                                                    }
                                                                    match guard.ui.pages.last_mut().unwrap() {
                                                                        crate::protocol::status::Page::Entry { selected, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            *selected = guard.ui.selected;
                                                                            *input_mode = guard.ui.input_mode;
                                                                            *input_editing = guard.ui.input_editing;
                                                                            *input_buffer = guard.ui.input_buffer.clone();
                                                                            *app_mode = guard.ui.app_mode;
                                                                        }
                                                                        crate::protocol::status::Page::Modbus { selected, subpage_active, subpage_form, subpage_tab_index, logs, log_selected, log_view_offset, log_auto_scroll, log_clear_pending, input_mode, input_editing, input_buffer, app_mode } => {
                                                                            *selected = guard.ui.selected;
                                                                            *subpage_active = guard.ui.subpage_active;
                                                                            *subpage_form = guard.ui.subpage_form.clone();
                                                                            *subpage_tab_index = guard.ui.subpage_tab_index;
                                                                            *logs = guard.ui.logs.clone();
                                                                            *log_selected = guard.ui.log_selected;
                                                                            *log_view_offset = guard.ui.log_view_offset;
                                                                            *log_auto_scroll = guard.ui.log_auto_scroll;
                                                                            *log_clear_pending = guard.ui.log_clear_pending;
                                                                            *input_mode = guard.ui.input_mode;
                                                                            *input_editing = guard.ui.input_editing;
                                                                            *input_buffer = guard.ui.input_buffer.clone();
                                                                            *app_mode = guard.ui.app_mode;
                                                                        }
                                                                    }
                                                                }
                                                            } else {
                                                                guard.ui.subpage_active = false;
                                                                guard.ui.subpage_form = None;
                                                                guard.ui.subpage_tab_index = crate::protocol::status::SubpageTab::Config;
                                                                crate::protocol::status::ui::ui_logs_set(guard, Vec::new());
                                                                crate::protocol::status::ui::ui_log_selected_set(guard, 0);
                                                                crate::protocol::status::ui::ui_log_view_offset_set(guard, 0);
                                                                crate::protocol::status::ui::ui_log_auto_scroll_set(guard, true);
                                                                guard.ui.input_mode = crate::protocol::status::InputMode::Ascii;
                                                                guard.ui.input_editing = false;
                                                                guard.ui.input_buffer.clear();
                                                                guard.ui.app_mode = crate::protocol::status::AppMode::Modbus;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::PageUp => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if is_log_tab(&guard) {
                                            // inline page_up
                                            if !crate::protocol::status::ui::ui_logs_get(guard)
                                                .is_empty()
                                            {
                                                let cur = crate::protocol::status::ui::ui_log_view_offset_get(guard);
                                                if cur > LOG_PAGE_JUMP {
                                                    crate::protocol::status::ui::ui_log_view_offset_set(guard, cur.saturating_sub(LOG_PAGE_JUMP));
                                                } else {
                                                    crate::protocol::status::ui::ui_log_view_offset_set(guard, 0);
                                                }
                                                crate::protocol::status::ui::ui_log_auto_scroll_set(
                                                    guard, false,
                                                );
                                            }
                                        } else {
                                            let about_idx =
                                                guard.ports.list.len().saturating_add(2);
                                            if guard.ui.subpage_active
                                                && guard.ui.selected == about_idx
                                            {
                                                guard.ports.about_view_offset = guard
                                                    .ports
                                                    .about_view_offset
                                                    .saturating_sub(LOG_PAGE_JUMP);
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::PageDown => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if is_log_tab(&guard) {
                                            // inline page_down
                                            if !crate::protocol::status::ui::ui_logs_get(guard)
                                                .is_empty()
                                            {
                                                let max_bottom =
                                                    crate::protocol::status::ui::ui_logs_get(guard)
                                                        .len()
                                                        .saturating_sub(1);
                                                let new_bottom = crate::protocol::status::ui::ui_log_view_offset_get(guard).saturating_add(LOG_PAGE_JUMP);
                                                let new_bottom =
                                                    std::cmp::min(max_bottom, new_bottom);
                                                crate::protocol::status::ui::ui_log_view_offset_set(
                                                    guard, new_bottom,
                                                );
                                                crate::protocol::status::ui::ui_log_auto_scroll_set(guard, crate::protocol::status::ui::ui_log_view_offset_get(guard) >= max_bottom);
                                            }
                                        } else {
                                            let about_idx =
                                                guard.ports.list.len().saturating_add(2);
                                            if guard.ui.subpage_active
                                                && guard.ui.selected == about_idx
                                            {
                                                guard.ports.about_view_offset = guard
                                                    .ports
                                                    .about_view_offset
                                                    .saturating_add(LOG_PAGE_JUMP);
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::JumpTop => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if is_log_tab(&guard) {
                                            crate::protocol::status::ui::ui_log_view_offset_set(
                                                guard, 0,
                                            );
                                            crate::protocol::status::ui::ui_log_auto_scroll_set(
                                                guard, false,
                                            );
                                        } else {
                                            let about_idx =
                                                guard.ports.list.len().saturating_add(2);
                                            if guard.ui.subpage_active
                                                && guard.ui.selected == about_idx
                                            {
                                                guard.ports.about_view_offset = 0;
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::JumpBottom => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if is_log_tab(&guard) {
                                            let total =
                                                crate::protocol::status::ui::ui_logs_get(guard)
                                                    .len();
                                            crate::protocol::status::ui::ui_log_view_offset_set(
                                                guard,
                                                if total > 0 { total - 1 } else { 0 },
                                            );
                                            crate::protocol::status::ui::ui_log_auto_scroll_set(
                                                guard, true,
                                            );
                                        } else {
                                            let about_idx =
                                                guard.ports.list.len().saturating_add(2);
                                            if guard.ui.subpage_active
                                                && guard.ui.selected == about_idx
                                            {
                                                guard.ports.about_view_offset = 0;
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::ToggleFollow => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if is_log_tab(&guard) {
                                            let cur =
                                                crate::protocol::status::ui::ui_log_auto_scroll_get(
                                                    guard,
                                                );
                                            crate::protocol::status::ui::ui_log_auto_scroll_set(
                                                guard, !cur,
                                            );
                                            if !cur {
                                                let total =
                                                    crate::protocol::status::ui::ui_logs_get(guard)
                                                        .len();
                                                crate::protocol::status::ui::ui_log_view_offset_set(
                                                    guard,
                                                    if total > 0 { total - 1 } else { 0 },
                                                );
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            // Removed SwitchMode / CycleMode / ShowModeSelector branches after unifying mode
                            Action::EnterSubpage(_) => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        let state = guard
                                            .ports
                                            .states
                                            .get(guard.ui.selected)
                                            .cloned()
                                            .unwrap_or(crate::protocol::status::PortState::Free);
                                        if state
                                            == crate::protocol::status::PortState::OccupiedByThis
                                        {
                                            crate::protocol::status::ui::ui_subpage_active_set(
                                                guard, true,
                                            );
                                            guard.init_subpage_form();
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::AddRegister => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if guard.ui.subpage_active {
                                            if guard.ui.subpage_form.is_none() {
                                                guard.init_subpage_form();
                                            }
                                            if let Some(form) = guard.ui.subpage_form.as_mut() {
                                                form.registers.push(crate::protocol::status::RegisterEntry {
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
                                            });
                                            }
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::DeleteRegister => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if let Some(form) = guard.ui.subpage_form.as_mut() {
                                            form.registers.pop();
                                        }
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::EditToggle => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if let Some(form) = guard.ui.subpage_form.as_mut() {
                                            form.editing = !form.editing;
                                            if form.editing {
                                                match form.cursor {
                                                0 => form.editing_field = Some(crate::protocol::status::EditingField::Loop),
                                                1 => { form.editing = false; form.editing_field = None; }
                                                2 => form.editing_field = Some(crate::protocol::status::EditingField::Baud),
                                                3 => form.editing_field = Some(crate::protocol::status::EditingField::Parity),
                                                4 => form.editing_field = Some(crate::protocol::status::EditingField::DataBits),
                                                5 => form.editing_field = Some(crate::protocol::status::EditingField::StopBits),
                                                6 => form.editing_field = Some(crate::protocol::status::EditingField::GlobalInterval),
                                                7 => form.editing_field = Some(crate::protocol::status::EditingField::GlobalTimeout),
                                                n => { let ridx = n.saturating_sub(8); form.editing_field = Some(crate::protocol::status::EditingField::RegisterField { idx: ridx, field: crate::protocol::status::RegisterField::SlaveId }); }
                                            }
                                                form.input_buffer.clear();
                                                if let Some(
                                                    crate::protocol::status::EditingField::Baud,
                                                ) = form.editing_field.clone()
                                                {
                                                    let presets: [u32; 8] = [
                                                        1200, 2400, 4800, 9600, 19200, 38400,
                                                        57600, 115200,
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
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::ExitSubpage => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        crate::protocol::status::ui::ui_subpage_active_set(
                                            guard, false,
                                        );
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::TogglePort => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        guard.toggle_selected_port();
                                        let state = guard
                                            .ports
                                            .states
                                            .get(guard.ui.selected)
                                            .cloned()
                                            .unwrap_or(crate::protocol::status::PortState::Free);
                                        if state
                                            != crate::protocol::status::PortState::OccupiedByThis
                                        { /* nothing */
                                        }
                                        Ok(())
                                    },
                                );
                            }
                            Action::SwitchNext => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::SwitchPrev => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::ClearError => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        crate::protocol::status::ui::ui_error_set(guard, None);
                                        Ok(())
                                    },
                                );
                            }
                            Action::QuickScan => {
                                let _ = crate::protocol::status::status_rw::write_status(
                                    &app,
                                    |guard| {
                                        if guard.ui.selected >= guard.ports.list.len()
                                            && guard.ui.selected == guard.ports.list.len()
                                        {
                                            // inline quick_scan -> perform_device_scan
                                            guard.scan.last_scan_info.clear();
                                            guard.scan.last_scan_time = Some(chrono::Local::now());
                                        }
                                        Ok(())
                                    },
                                );
                            }
                            Action::None => {}
                        }
                    }
                }
            }
        }
    }

    terminal.clear()?;
    Ok(())
}
