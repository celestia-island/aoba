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

                match map_key(key.code) {
                    Action::Quit => break,
                    Action::FocusLeft => {
                        if let Ok(mut guard) = app.lock() {
                            guard.focus = Focus::Left;
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for FocusLeft");
                        }
                    }
                    Action::FocusRight => {
                        if let Ok(mut guard) = app.lock() {
                            // Only allow focusing right when the selected port is occupied by this app
                            let state = guard
                                .port_states
                                .get(guard.selected)
                                .cloned()
                                .unwrap_or(crate::protocol::status::PortState::Free);
                            if state == crate::protocol::status::PortState::OccupiedByThis {
                                guard.focus = Focus::Right;
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for FocusRight");
                        }
                    }
                    Action::MoveNext => {
                        if let Ok(mut guard) = app.lock() {
                            if matches!(guard.focus, Focus::Left) {
                                guard.next();
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for MoveNext");
                        }
                    }
                    Action::MovePrev => {
                        if let Ok(mut guard) = app.lock() {
                            if matches!(guard.focus, Focus::Left) {
                                guard.prev();
                            }
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for MovePrev");
                        }
                    }
                    Action::Refresh => {
                        if let Ok(mut guard) = app.lock() {
                            guard.refresh();
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for Refresh");
                        }
                    }
                    Action::SwitchMode(i) => {
                        if let Ok(mut guard) = app.lock() {
                            let mode = match i {
                                0 => RightMode::Master,
                                1 => RightMode::SlaveStack,
                                _ => RightMode::Listen,
                            };
                            guard.right_mode = mode;
                            guard.clear_error();
                        } else {
                            log::error!("[TUI] failed to lock app for SwitchMode");
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
