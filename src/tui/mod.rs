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

pub fn start() -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");

    // Setup terminal
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = Arc::new(RwLock::new(Status::new()));

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
                        _ => todo!("handle UI -> core message: {:?}", msg),
                    }
                }

                let _ = core_tx.send(CoreToUi::Tick);
                thread::sleep(Duration::from_millis(50));
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
    _app: Arc<RwLock<Status>>,
    bus: crate::tui::utils::bus::Bus,
) -> Result<()> {
    loop {
        // First try to receive a notification from core thread (short timeout) to reduce busy waiting
        let _ = bus.core_rx.recv_timeout(Duration::from_millis(50));

        // Poll for input
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                if key.kind != crossterm::event::KeyEventKind::Press {
                    continue; // Ignore non-initial key press (repeat / release)
                }

                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    if let crossterm::event::KeyCode::Char('c') = key.code {
                        let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Quit);
                        break;
                    }
                }
            }
        }
    }

    terminal.clear()?;
    Ok(())
}
