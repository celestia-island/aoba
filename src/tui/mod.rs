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

                // If a subpage form is active and in editing mode, capture raw character input
                // and interpret it as form input. Otherwise map keys to high-level actions.
                let lock = app.lock();
                let _is_editing = match &lock {
                    Ok(g) => g.subpage_form.as_ref().map(|f| f.editing).unwrap_or(false),
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
                                guard.mode_selector_active = false;
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
                    // handle editing keys explicitly
                    use crossterm::event::KeyCode as KC;
                    if let Ok(mut guard) = app.lock() {
                        if let Some(form) = guard.subpage_form.as_mut() {
                            match key.code {
                                KC::Char(c) => {
                                    // append printable characters
                                    form.input_buffer.push(c);
                                }
                                KC::Backspace => {
                                    form.input_buffer.pop();
                                }
                                KC::Enter => {
                                    // apply buffer to the currently editing field
                                    if let Some(field) = &form.editing_field {
                                        match field {
                                            crate::protocol::status::EditingField::Baud => {
                                                if let Ok(v) = form.input_buffer.parse::<u32>() {
                                                    form.baud = v;
                                                }
                                            }
                                            crate::protocol::status::EditingField::StopBits => {
                                                if let Ok(v) = form.input_buffer.parse::<u8>() {
                                                    form.stop_bits = v;
                                                }
                                            }
                                            crate::protocol::status::EditingField::Parity => {
                                                // simple mapping by text
                                                match form.input_buffer.as_str() {
                                                    "None" | "none" | "N" | "n" => {
                                                        form.parity = crate::protocol::status::Parity::None
                                                    }
                                                    "Even" | "even" | "E" | "e" => {
                                                        form.parity = crate::protocol::status::Parity::Even
                                                    }
                                                    "Odd" | "odd" | "O" | "o" => {
                                                        form.parity = crate::protocol::status::Parity::Odd
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            crate::protocol::status::EditingField::RegisterField { idx, field } => {
                                                if let Some(reg) = form.registers.get_mut(*idx) {
                                                    match field {
                                                        crate::protocol::status::RegisterField::SlaveId => {
                                                            if let Ok(v) = form.input_buffer.parse::<u8>() {
                                                                reg.slave_id = v;
                                                            }
                                                        }
                                                        crate::protocol::status::RegisterField::Mode => {
                                                            if let Ok(v) = form.input_buffer.parse::<u8>() {
                                                                reg.mode = v;
                                                            }
                                                        }
                                                        crate::protocol::status::RegisterField::Address => {
                                                            if let Ok(v) = form.input_buffer.parse::<u16>() {
                                                                reg.address = v;
                                                            }
                                                        }
                                                        crate::protocol::status::RegisterField::Length => {
                                                            if let Ok(v) = form.input_buffer.parse::<u16>() {
                                                                reg.length = v;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // exit editing state for the field but keep overall editing flag
                                    form.input_buffer.clear();
                                    form.editing_field = None;
                                }
                                KC::Esc => {
                                    // cancel current field editing
                                    form.input_buffer.clear();
                                    form.editing_field = None;
                                }
                                _ => {}
                            }
                        }
                        guard.clear_error();
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
                                    let total = 3usize.saturating_add(form.registers.len());
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
                                    let total = 3usize.saturating_add(form.registers.len());
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
                                    // choose field based on cursor: 0=baud,1=parity,2=stopbits,>=3 -> register index
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
                                                crate::protocol::status::EditingField::StopBits,
                                            )
                                        }
                                        n => {
                                            let ridx = n.saturating_sub(3);
                                            form.editing_field = Some(crate::protocol::status::EditingField::RegisterField { idx: ridx, field: crate::protocol::status::RegisterField::SlaveId });
                                        }
                                    }
                                    form.input_buffer.clear();
                                } else {
                                    form.editing_field = None;
                                    form.input_buffer.clear();
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
