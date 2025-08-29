pub mod input;
pub mod ui;

use anyhow::Result;
use std::{
    io::{self, Stdout},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use ratatui::{backend::CrosstermBackend, prelude::*};

use crate::protocol::status::RightMode;
use crate::{
    protocol::status::{Focus, Status},
    tui::input::{map_key, Action},
};

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

    // Background refresher thread
    {
        let app_clone = Arc::clone(&app);
        thread::spawn(move || loop {
            thread::sleep(std::time::Duration::from_secs(3));
            if let Ok(mut guard) = app_clone.lock() {
                // Always refresh to detect added/removed COM ports
                guard.refresh();
            } else {
                log::error!("[TUI] refresher thread: failed to lock app (poisoned)");
            }
        });
    }

    let res = run_app(&mut terminal, Arc::clone(&app));

    // Restore terminal
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::LeaveAlternateScreen)?;
    crossterm::terminal::disable_raw_mode()?;

    res
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<&mut Stdout>>,
    app: Arc<Mutex<Status>>,
) -> Result<()> {
    loop {
        // Draw with short-lived lock
        {
            match app.lock() {
                Ok(guard) => {
                    terminal.draw(|f| crate::tui::ui::render_ui(f, &*guard))?;
                }
                Err(_) => {
                    log::error!("[TUI] failed to lock app for drawing (poisoned)");
                    // cannot set app.error because lock failed; just continue
                }
            }
        }

        // Poll for input
        if crossterm::event::poll(Duration::from_millis(200))? {
            let evt = match crossterm::event::read() {
                Ok(e) => e,
                Err(e) => {
                    if let Ok(mut guard) = app.lock() {
                        guard.set_error(format!("input read error: {}", e));
                    } else {
                        log::error!("[TUI] input read error: {}", e);
                    }
                    continue;
                }
            };

            if let crossterm::event::Event::Key(key) = evt {
                // Only handle the initial key press event. Ignore Repeat and Release
                // events so a single physical key press maps to a single action.
                match key.kind {
                    crossterm::event::KeyEventKind::Press => {}
                    crossterm::event::KeyEventKind::Repeat => continue,
                    _ => continue, // Release or other kinds
                }

                // If a subpage form is active and in editing mode OR we're on the
                // Baud->Custom pending slot, capture raw character input and
                // interpret it as form input. Otherwise map keys to high-level actions.
                let lock = app.lock();
                let _is_editing = match &lock {
                    Ok(g) => g
                        .subpage_form
                        .as_ref()
                        .map(|f| {
                            if f.editing {
                                true
                            } else {
                                // allow pre-confirm typing when editing_field is Baud and the current choice is Custom
                                if let Some(crate::protocol::status::EditingField::Baud) =
                                    f.editing_field.clone()
                                {
                                    let presets: [u32; 8] =
                                        [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                                    let custom_idx = presets.len();
                                    let cur = f.edit_choice_index.unwrap_or_else(|| {
                                        presets
                                            .iter()
                                            .position(|&p| p == f.baud)
                                            .unwrap_or(custom_idx)
                                    });
                                    cur == custom_idx
                                } else {
                                    false
                                }
                            }
                        })
                        .unwrap_or(false),
                    Err(_) => false,
                };

                // If mode selector overlay is active, handle its navigation here
                let mode_selector_active = match &lock {
                    Ok(g) => g.mode_selector_active,
                    Err(_) => false,
                };

                // unlock drop of 'lock' to avoid double-lock later
                drop(lock);

                if mode_selector_active {
                    use crossterm::event::KeyCode as KC;
                    if let Ok(mut guard) = app.lock() {
                        match key.code {
                            KC::Up | KC::Char('k') => {
                                if guard.mode_selector_index == 0 {
                                    guard.mode_selector_index = 1;
                                } else {
                                    guard.mode_selector_index -= 1;
                                }
                            }
                            KC::Down | KC::Char('j') => {
                                guard.mode_selector_index = (guard.mode_selector_index + 1) % 2;
                            }
                            KC::Enter => {
                                // apply selection
                                guard.right_mode = match guard.mode_selector_index {
                                    0 => RightMode::Master,
                                    1 => RightMode::SlaveStack,
                                    _ => RightMode::Master,
                                };
                                // If currently in a subpage, switch the active subpage view but preserve existing form data
                                if guard.active_subpage.is_some() {
                                    guard.active_subpage = Some(guard.right_mode);
                                }
                            }
                            KC::Esc => {
                                guard.mode_selector_active = false;
                            }
                            _ => {}
                        }
                        guard.clear_error();
                    }
                    // skip normal key handling while selector active
                    continue;
                }

                // re-evaluate editing after potential selector handling
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
                                        // pre-confirm case: assume Baud custom pending -> accept digits only
                                        if c.is_ascii_digit() {
                                            form.input_buffer.push(c);
                                        }
                                    }
                                }
                                KC::Backspace => {
                                    form.input_buffer.pop();
                                }
                                KC::Left | KC::Right => {
                                    // try to interpret and adjust current field numerically or toggle parity
                                    if let Some(field) = &form.editing_field {
                                        let dir: i64 = match key.code {
                                            KC::Left => -1,
                                            KC::Right => 1,
                                            _ => 0,
                                        };
                                        match field {
                                            crate::protocol::status::EditingField::Baud => {
                                                let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                                                let custom_idx = presets.len();
                                                // initialize edit_choice_index if missing
                                                if form.edit_choice_index.is_none() {
                                                    let idx = presets.iter().position(|&p| p == form.baud).unwrap_or(custom_idx);
                                                    form.edit_choice_index = Some(idx);
                                                }
                                                if let Some(mut idx) = form.edit_choice_index {
                                                    if dir > 0 {
                                                        // move right
                                                        if idx >= custom_idx {
                                                            idx = 0;
                                                        } else {
                                                            idx = idx + 1;
                                                        }
                                                    } else {
                                                        // move left
                                                        if idx == 0 {
                                                            idx = custom_idx;
                                                        } else {
                                                            idx = idx - 1;
                                                        }
                                                    }
                                                    form.edit_choice_index = Some(idx);
                                                    // if moved to preset, clear buffer and update baud preview
                                                    if idx < presets.len() {
                                                        form.input_buffer.clear();
                                                        form.baud = presets[idx];
                                                    }
                                                }
                                            }
                                            crate::protocol::status::EditingField::StopBits => {
                                                // cycle among 1, 2
                                                let options = [1, 2];
                                                let cur_idx = options.iter().position(|&v| v == form.stop_bits).unwrap_or(0);
                                                let next = if dir > 0 { (cur_idx + 1) % options.len() } else { (cur_idx + options.len() - 1) % options.len() };
                                                form.stop_bits = options[next];
                                            }
                                            crate::protocol::status::EditingField::Parity => {
                                                // cycle parity options
                                                let options = [crate::protocol::status::Parity::None, crate::protocol::status::Parity::Even, crate::protocol::status::Parity::Odd];
                                                let idx = options.iter().position(|&p| p == form.parity).unwrap_or(0);
                                                let next = if dir > 0 { (idx + 1) % options.len() } else { (idx + options.len() - 1) % options.len() };
                                                form.parity = options[next].clone();
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
                                                            let new = (reg.slave_id as i64).saturating_add(dir);
                                                            if new >= 0 && new <= u8::MAX as i64 {
                                                                reg.slave_id = new as u8;
                                                            }
                                                        }
                                                        crate::protocol::status::RegisterField::Mode => {
                                                            let new = (reg.mode as i64).saturating_add(dir);
                                                            if new >= 0 && new <= u8::MAX as i64 {
                                                                reg.mode = new as u8;
                                                            }
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
                                    // If we're in Baud field and currently on Custom but not yet confirmed,
                                    // confirm deeper edit instead of committing immediately.
                                    if let Some(crate::protocol::status::EditingField::Baud) =
                                        &form.editing_field
                                    {
                                        let presets: [u32; 8] =
                                            [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                                        let custom_idx = presets.len();
                                        if let Some(idx) = form.edit_choice_index {
                                            if idx == custom_idx && !form.edit_confirmed {
                                                // enter deeper confirmed edit stage
                                                form.edit_confirmed = true;
                                                form.editing = true;
                                                // keep input_buffer as is (user may have typed)
                                                continue;
                                            }
                                        }
                                    }
                                    // commit and exit field editing
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
                                                        // custom: must parse and validate [1200..=2_000_000]
                                                        if !form.input_buffer.is_empty() {
                                                            if let Ok(v) =
                                                                form.input_buffer.parse::<u32>()
                                                            {
                                                                if v >= 1200 && v <= 2_000_000 {
                                                                    form.baud = v;
                                                                } else {
                                                                    pending_error = Some(
                                                                        crate::i18n::lang()
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
                                                    // fallback: if buffer present try parse and validate
                                                    if !form.input_buffer.is_empty() {
                                                        if let Ok(v) =
                                                            form.input_buffer.parse::<u32>()
                                                        {
                                                            if v >= 1200 && v <= 2_000_000 {
                                                                form.baud = v;
                                                            } else {
                                                                pending_error = Some(
                                                                    crate::i18n::lang()
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
                                            _ => {}
                                        }
                                    }
                                    // exit current field editing and leave overall editing mode only when commit succeeded
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
                                    // cancel current field editing (revert input buffer)
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
                    continue; // skip normal key mapping when editing
                }

                // If a subpage is active, give it first chance to consume the key
                if let Ok(mut guard) = app.lock() {
                    if crate::tui::ui::pages::handle_key_in_subpage(key, &mut *guard) {
                        guard.clear_error();
                        continue; // consumed by subpage
                    }
                }

                match map_key(key.code) {
                    Action::Quit => break,
                    Action::LeavePage => {
                        if let Ok(mut guard) = app.lock() {
                            // If a subpage is active, Exit it; otherwise focus left
                            if guard.active_subpage.is_some() {
                                // Exit subpage and keep focus on Left
                                guard.active_subpage = None;
                                guard.focus = Focus::Left;
                            } else {
                                // Ensure focus stays Left (do not allow other keys to move focus to Right)
                                guard.focus = Focus::Left;
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for FocusLeft");
                        }
                    }
                    Action::EnterPage => {
                        if let Ok(mut guard) = app.lock() {
                            // If selected port is occupied by this app, entering Right now opens the subpage for the current right_mode
                            let state = guard
                                .port_states
                                .get(guard.selected)
                                .cloned()
                                .unwrap_or(crate::protocol::status::PortState::Free);
                            if state == crate::protocol::status::PortState::OccupiedByThis {
                                // activate subpage corresponding to current right_mode
                                guard.active_subpage = Some(guard.right_mode);
                                // reset subpage tab index and prepare form data
                                guard.subpage_tab_index = 0;
                                guard.init_subpage_form();
                                // keep focus on Left logically (right content is a subpage overlay)
                                guard.focus = Focus::Left;
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for FocusRight");
                        }
                    }
                    Action::MoveNext => {
                        if let Ok(mut guard) = app.lock() {
                            if guard.active_subpage.is_some() {
                                if let Some(form) = guard.subpage_form.as_mut() {
                                    let total = 4usize.saturating_add(form.registers.len());
                                    if total > 0 {
                                        form.cursor = (form.cursor + 1) % total;
                                    }
                                }
                            } else if matches!(guard.focus, Focus::Left) {
                                guard.next();
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for MoveNext");
                        }
                    }
                    Action::MovePrev => {
                        if let Ok(mut guard) = app.lock() {
                            if guard.active_subpage.is_some() {
                                if let Some(form) = guard.subpage_form.as_mut() {
                                    let total = 4usize.saturating_add(form.registers.len());
                                    if total > 0 {
                                        if form.cursor == 0 {
                                            form.cursor = total - 1;
                                        } else {
                                            form.cursor -= 1;
                                        }
                                    }
                                }
                            } else if matches!(guard.focus, Focus::Left) {
                                guard.prev();
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for MovePrev");
                        }
                    }
                    // Refresh is handled by background thread; removed from key mapping
                    Action::SwitchMode(i) => {
                        if let Ok(mut guard) = app.lock() {
                            let mode = match i {
                                0 => RightMode::Master,
                                1 => RightMode::SlaveStack,
                                _ => RightMode::Listen,
                            };
                            guard.right_mode = mode;
                            if guard.active_subpage.is_some() {
                                guard.active_subpage = Some(guard.right_mode);
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for SwitchMode");
                        }
                    }
                    Action::CycleMode => {
                        if let Ok(mut guard) = app.lock() {
                            guard.right_mode = match guard.right_mode {
                                RightMode::Master => RightMode::SlaveStack,
                                RightMode::SlaveStack => RightMode::Listen,
                                RightMode::Listen => RightMode::Master,
                            };
                            if guard.active_subpage.is_some() {
                                guard.active_subpage = Some(guard.right_mode);
                            }
                            guard.clear_error();
                        }
                    }
                    Action::ShowModeSelector => {
                        if let Ok(mut guard) = app.lock() {
                            // only allow mode selector when no subpage overlay is active
                            if guard.active_subpage.is_none() {
                                // only for ports owned by this app
                                let state = guard
                                    .port_states
                                    .get(guard.selected)
                                    .cloned()
                                    .unwrap_or(crate::protocol::status::PortState::Free);
                                if state == crate::protocol::status::PortState::OccupiedByThis {
                                    guard.mode_selector_active = true;
                                    guard.mode_selector_index = match guard.right_mode {
                                        RightMode::Master => 0,
                                        RightMode::SlaveStack => 1,
                                        RightMode::Listen => 0,
                                    };
                                }
                            }
                            guard.clear_error();
                        }
                    }
                    Action::EnterSubpage(ch) => {
                        if let Ok(mut guard) = app.lock() {
                            // only when selected port is occupied by this app
                            let state = guard
                                .port_states
                                .get(guard.selected)
                                .cloned()
                                .unwrap_or(crate::protocol::status::PortState::Free);
                            if state == crate::protocol::status::PortState::OccupiedByThis {
                                match ch {
                                    'p' => guard.active_subpage = Some(RightMode::SlaveStack), // pull -> 从站
                                    's' => guard.active_subpage = Some(RightMode::Master), // slave -> 主站 (named for historical reasons)
                                    _ => {}
                                }
                                // initialize form
                                guard.init_subpage_form();
                            }
                            guard.clear_error();
                        }
                    }
                    Action::AddRegister => {
                        if let Ok(mut guard) = app.lock() {
                            if guard.active_subpage.is_some() {
                                if guard.subpage_form.is_none() {
                                    guard.init_subpage_form();
                                }
                                if let Some(form) = guard.subpage_form.as_mut() {
                                    form.registers.push(crate::protocol::status::RegisterEntry {
                                        slave_id: 1,
                                        mode: 1,
                                        address: 0,
                                        length: 1,
                                    });
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
                                    // choose field based on cursor: 0=baud,1=parity,2=databits,3=stopbits,>=4 -> register index
                                    match form.cursor {
                                        0 => {
                                            form.editing_field =
                                                Some(crate::protocol::status::EditingField::Baud)
                                        }
                                        1 => {
                                            form.editing_field =
                                                Some(crate::protocol::status::EditingField::Parity)
                                        }
                                        2 => {
                                            form.editing_field = Some(
                                                crate::protocol::status::EditingField::DataBits,
                                            )
                                        }
                                        3 => {
                                            form.editing_field = Some(
                                                crate::protocol::status::EditingField::StopBits,
                                            )
                                        }
                                        n => {
                                            let ridx = n.saturating_sub(4);
                                            form.editing_field = Some(crate::protocol::status::EditingField::RegisterField { idx: ridx, field: crate::protocol::status::RegisterField::SlaveId });
                                        }
                                    }
                                    form.input_buffer.clear();
                                    // If entering Baud edit, initialize edit_choice_index and prefill buffer for custom
                                    if let Some(crate::protocol::status::EditingField::Baud) =
                                        form.editing_field.clone()
                                    {
                                        let presets: [u32; 8] =
                                            [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                                        let _custom_idx = presets.len();
                                        let idx = presets
                                            .iter()
                                            .position(|&p| p == form.baud)
                                            .unwrap_or(_custom_idx);
                                        form.edit_choice_index = Some(idx);
                                        if idx == presets.len() {
                                            form.input_buffer = form.baud.to_string();
                                        }
                                        form.edit_confirmed = false; // not yet confirmed deeper edit
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
                    // StartEditField removed: editing is handled via EditToggle and direct editing flow
                    // EditCancel removed: exiting edit is handled by existing edit flow and Esc handling
                    Action::ExitSubpage => {
                        if let Ok(mut guard) = app.lock() {
                            guard.active_subpage = None;
                            guard.clear_error();
                        }
                    }
                    Action::TogglePort => {
                        if let Ok(mut guard) = app.lock() {
                            guard.toggle_selected_port();
                            // ensure focus is valid: only allow Right focus when selected port is OccupiedByThis
                            let state = guard
                                .port_states
                                .get(guard.selected)
                                .cloned()
                                .unwrap_or(crate::protocol::status::PortState::Free);
                            if state != crate::protocol::status::PortState::OccupiedByThis {
                                guard.focus = Focus::Left;
                            }
                        } else {
                            log::error!("[TUI] failed to lock app for TogglePort");
                        }
                    }
                    Action::SwitchNext => {
                        if let Ok(mut guard) = app.lock() {
                            guard.right_mode = match guard.right_mode {
                                RightMode::Master => RightMode::SlaveStack,
                                RightMode::SlaveStack => RightMode::Listen,
                                RightMode::Listen => RightMode::Master,
                            };
                            if guard.active_subpage.is_some() {
                                guard.active_subpage = Some(guard.right_mode);
                            }
                            guard.clear_error();
                        }
                    }
                    Action::SwitchPrev => {
                        if let Ok(mut guard) = app.lock() {
                            guard.right_mode = match guard.right_mode {
                                RightMode::Master => RightMode::Listen,
                                RightMode::SlaveStack => RightMode::Master,
                                RightMode::Listen => RightMode::SlaveStack,
                            };
                            if guard.active_subpage.is_some() {
                                guard.active_subpage = Some(guard.right_mode);
                                guard.init_subpage_form();
                            }
                            guard.clear_error();
                        }
                    }
                    // ToggleAutoRefresh removed: auto-refresh is always enabled by background thread
                    Action::ClearError => {
                        if let Ok(mut guard) = app.lock() {
                            guard.clear_error();
                        }
                    }
                    Action::None => {}
                }
            }
        }

        // No automatic error clearing; errors are cleared manually via the UI
    }

    terminal.clear()?;
    Ok(())
}
