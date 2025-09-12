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

use crate::protocol::status::Status;
use crate::tui::utils::bus::{Bus, CoreToUi, UiToCore};

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

    // Create channels for three-thread architecture
    let (core_tx, core_rx) = flume::unbounded::<CoreToUi>(); // core -> ui
    let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>(); // ui -> core
    let bus = Bus::new(core_rx.clone(), ui_tx.clone());

    // Thread 1: Core processing thread - handles UiToCore and CoreToUi communication
    {
        let _app_clone = Arc::clone(&app);
        let core_tx_clone = core_tx.clone();
        thread::spawn(move || {
            loop {
                // Handle commands coming from UI
                while let Ok(msg) = ui_rx.try_recv() {
                    match msg {
                        UiToCore::Quit => {
                            log::info!("[CORE] Received quit signal");
                            return;
                        }
                        UiToCore::Refresh => {
                            // TODO: Handle refresh logic
                            log::debug!("[CORE] Refresh requested");
                        }
                        UiToCore::PausePolling => {
                            // TODO: Handle pause polling
                            log::debug!("[CORE] Pause polling requested");
                        }
                        UiToCore::ResumePolling => {
                            // TODO: Handle resume polling
                            log::debug!("[CORE] Resume polling requested");
                        }
                    }
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
                                key, &app_read, &bus_clone
                            );
                            
                            if !consumed {
                                // Handle global keys
                                match key.code {
                                    crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Char('Q') => {
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
