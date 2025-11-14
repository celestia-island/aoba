use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use std::{io, sync::Arc, time::Duration};

use ratatui::{backend::CrosstermBackend, layout::*, prelude::*};

use crate::{tui::status::Status, utils::sleep_1s};

/// Render UI function that only reads from Status (immutable reference)
fn render_ui(frame: &mut Frame) -> Result<()> {
    let area = frame.area();

    let mut hints_count = match crate::tui::ui::pages::bottom_hints_for_app() {
        Ok(h) => h.len(),
        Err(_) => 0,
    };

    let error_visible = crate::tui::ui::bottom::visible_error()?.is_some();
    if error_visible {
        hints_count += 1; // dismiss hint row is appended to bottom hints
    }

    let bottom_height = hints_count + if error_visible { 1 } else { 0 };

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(3),    // main
            Constraint::Length(bottom_height as u16),
        ])
        .split(area);

    crate::tui::ui::title::render_title(frame, main_chunks[0])?;
    crate::tui::ui::pages::render_panels(frame, main_chunks[1])?;
    crate::tui::ui::bottom::render_bottom(frame, main_chunks[2])?;

    Ok(())
}

#[allow(dead_code)]
#[doc(hidden)]
pub fn render_ui_for_testing(frame: &mut Frame) -> Result<()> {
    render_ui(frame)
}

pub(crate) fn run_rendering_loop(
    bus: crate::core::bus::Bus,
    thr_rx: flume::Receiver<anyhow::Result<()>>,
) -> Result<()> {
    // Initialize terminal inside rendering thread to avoid cross-thread Terminal usage
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        loop {
            if let Ok(res) = thr_rx.try_recv() {
                if let Err(err) = res {
                    eprintln!("thread exited with error: {err:#}");
                    return Err(err);
                } else {
                    log::info!("a monitored thread exited cleanly; shutting down");
                    return Ok(());
                }
            }

            let should_quit = !matches!(
                bus.core_rx.recv_timeout(Duration::from_millis(100)),
                Ok(crate::core::bus::CoreToUi::Tick)
                    | Ok(crate::core::bus::CoreToUi::Refreshed)
                    | Ok(crate::core::bus::CoreToUi::Error)
                    | Err(flume::RecvTimeoutError::Timeout)
            );

            if should_quit {
                break;
            }

            terminal.draw(|frame| {
                render_ui(frame).expect("Render failed");
            })?;
        }

        terminal.clear()?;
        Ok(())
    })();

    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;

    result
}

fn parse_key_string(key: &str) -> Result<crossterm::event::Event> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    let (code, modifiers) = if let Some(rest) = key.strip_prefix("Ctrl+") {
        match rest {
            "c" => (KeyCode::Char('c'), KeyModifiers::CONTROL),
            "s" => (KeyCode::Char('s'), KeyModifiers::CONTROL),
            "a" => (KeyCode::Char('a'), KeyModifiers::CONTROL),
            "Esc" => (KeyCode::Esc, KeyModifiers::CONTROL),
            "PageUp" => (KeyCode::PageUp, KeyModifiers::CONTROL),
            _ => return Err(anyhow!("Unsupported Ctrl+ combination: {rest}")),
        }
    } else {
        match key {
            "Enter" => (KeyCode::Enter, KeyModifiers::NONE),
            "Esc" | "Escape" => (KeyCode::Esc, KeyModifiers::NONE),
            "Backspace" => (KeyCode::Backspace, KeyModifiers::NONE),
            "Tab" => (KeyCode::Tab, KeyModifiers::NONE),
            "Up" => (KeyCode::Up, KeyModifiers::NONE),
            "Down" => (KeyCode::Down, KeyModifiers::NONE),
            "Left" => (KeyCode::Left, KeyModifiers::NONE),
            "Right" => (KeyCode::Right, KeyModifiers::NONE),
            "PageUp" => (KeyCode::PageUp, KeyModifiers::NONE),
            "PageDown" => (KeyCode::PageDown, KeyModifiers::NONE),
            "Home" => (KeyCode::Home, KeyModifiers::NONE),
            "End" => (KeyCode::End, KeyModifiers::NONE),
            _ if key.starts_with("Char(") && key.ends_with(")") => {
                let ch = key[5..key.len() - 1]
                    .chars()
                    .next()
                    .ok_or_else(|| anyhow!("Empty Char() specification"))?;
                (KeyCode::Char(ch), KeyModifiers::NONE)
            }
            _ if key.len() == 1 => {
                let ch = key.chars().next().unwrap();
                (KeyCode::Char(ch), KeyModifiers::NONE)
            }
            _ => return Err(anyhow!("Unsupported key string: {key}")),
        }
    };

    Ok(Event::Key(KeyEvent::new(code, modifiers)))
}

pub(crate) fn run_screen_capture_mode() -> Result<()> {
    log::info!("📸 Starting screen capture mode");

    let app = Arc::new(RwLock::new(Status::default()));
    crate::tui::status::init_status(app.clone())?;

    let status_path = std::path::Path::new("/tmp/status.json");
    if status_path.exists() {
        log::info!(
            "📄 Loading status from {path}",
            path = status_path.display()
        );
        let status_content = std::fs::read_to_string(status_path)?;
        let serializable_status: crate::tui::status::serializable::TuiStatus =
            serde_json::from_str(&status_content)?;

        crate::tui::status::write_status(|status| {
            serializable_status.apply_to_status(status)?;
            log::info!(
                "✅ Status loaded from file ({} ports)",
                serializable_status.ports.len()
            );
            Ok(())
        })?;
        log::info!("✅ Status loaded successfully");
    } else {
        log::warn!(
            "⚠️  No status file found at {}, using default state",
            status_path.display()
        );
    }

    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| {
        if let Err(err) = render_ui(frame) {
            log::error!("Failed to render UI: {err}");
        }
    })?;

    use std::io::Write;
    io::stdout().flush()?;

    log::info!("✅ Screen rendered, waiting for termination signal...");

    use crossterm::event::{Event, KeyCode, KeyModifiers};
    loop {
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
                    || (key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    break;
                }
            }
        }
    }

    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;

    log::info!("✅ Screen capture completed successfully");
    Ok(())
}

pub(crate) async fn start_with_ipc(_matches: &clap::ArgMatches, channel_id: &str) -> Result<()> {
    use ratatui::backend::TestBackend;

    log::info!("🔧 Starting TUI in IPC mode with channel ID: {channel_id}");

    let app = Arc::new(RwLock::new(Status::default()));
    crate::tui::status::init_status(app.clone())?;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;

    let (core_tx, core_rx) = flume::unbounded::<crate::core::bus::CoreToUi>();
    let (ui_tx, ui_rx) = flume::unbounded::<crate::core::bus::UiToCore>();

    let (input_kill_tx, _input_kill_rx) = flume::bounded::<()>(1);

    let core_task = tokio::spawn({
        let core_tx = core_tx.clone();
        let ui_rx = ui_rx.clone();

        async move {
            let _ = crate::tui::runtime::run_core_thread(ui_rx, core_tx, input_kill_tx).await;
        }
    });

    let ipc_channel_id = crate::utils::IpcChannelId(channel_id.to_string());
    log::info!("🔌 Creating IPC receiver...");
    let mut receiver = match crate::utils::IpcReceiver::new(ipc_channel_id.clone()).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("❌ Failed to create IPC receiver: {e}");
            return Err(e);
        }
    };

    let bus = crate::core::bus::Bus::new(core_rx.clone(), ui_tx.clone());

    log::info!("🔄 Starting IPC message loop");
    loop {
        match receiver.receive().await {
            Ok(crate::utils::E2EToTuiMessage::KeyPress { key }) => {
                log::info!("⌨️  Processing key press: {key}");
                if let Ok(event) = parse_key_string(&key) {
                    if let Err(err) = crate::tui::input::handle_event(event, &bus) {
                        log::warn!("Failed to handle key event: {err}");
                    }
                    sleep_1s().await;
                }
            }
            Ok(crate::utils::E2EToTuiMessage::CharInput { ch }) => {
                log::info!("📝 Processing char input: {ch}");
                let event = crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(ch),
                    crossterm::event::KeyModifiers::NONE,
                ));
                if let Err(err) = crate::tui::input::handle_event(event, &bus) {
                    log::warn!("Failed to handle char input: {err}");
                }
                sleep_1s().await;
            }
            Ok(crate::utils::E2EToTuiMessage::RequestScreen) => {
                log::info!("🖼️  Rendering screen to TestBackend");

                while let Ok(_msg) = bus.core_rx.try_recv() {}

                terminal
                    .draw(|frame| {
                        if let Err(err) = render_ui(frame) {
                            log::error!("Render error: {err}");
                        }
                    })
                    .map_err(|e| anyhow!("Failed to draw: {e}"))?;

                let buffer = terminal.backend().buffer();
                let area = buffer.area();
                let width = area.width;
                let height = area.height;

                let mut content = String::new();
                for y in 0..height {
                    for x in 0..width {
                        let cell = &buffer[(x, y)];
                        content.push_str(cell.symbol());
                    }
                    if y < height - 1 {
                        content.push('\n');
                    }
                }

                let response = crate::utils::TuiToE2EMessage::ScreenContent {
                    content,
                    width,
                    height,
                };

                if let Err(err) = receiver.send(response).await {
                    log::error!("Failed to send screen content: {err}");
                } else {
                    log::info!("📤 Sent screen content");
                }
            }
            Ok(crate::utils::E2EToTuiMessage::Shutdown) => {
                log::info!("🛑 Received shutdown message");
                break;
            }
            Err(err) => {
                log::error!("IPC receive error: {err}");
                break;
            }
        }
    }

    log::info!("🧹 Cleaning up IPC mode");
    ui_tx.send(crate::core::bus::UiToCore::Quit)?;
    core_task
        .await
        .map_err(|err| anyhow!("Failed to join core task: {err:?}"))?;

    Ok(())
}
