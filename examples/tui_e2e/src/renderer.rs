//! TUI renderer for TestBackend
//!
//! This module provides utilities to render the TUI application to a `TestBackend`
//! for snapshot testing, without needing to spawn a real process.

use anyhow::{bail, Result};

use ratatui::{backend::TestBackend, Terminal};

use aoba_ci_utils::{E2EToTuiMessage, IpcSender, TuiToE2EMessage};

/// Render the TUI via IPC by requesting screen content from the TUI process
///
/// This function is used in DrillDown mode to get the current screen content
/// from the running TUI process via IPC.
///
/// # Arguments
/// * `sender` - Mutable reference to the IPC sender
///
/// # Returns
/// A tuple containing the screen content string, width, and height
pub async fn render_tui_via_ipc(sender: &mut IpcSender) -> Result<(String, u16, u16)> {
    // Request screen content from TUI
    sender.send(E2EToTuiMessage::RequestScreen).await?;

    // Wait for response
    match sender.receive().await? {
        TuiToE2EMessage::ScreenContent {
            content,
            width,
            height,
        } => Ok((content, width, height)),
        TuiToE2EMessage::Error { message } => bail!("TUI returned error: {message}"),
        other => bail!("Unexpected response from TUI: {other:?}"),
    }
}

/// Render the TUI to a TestBackend with specified dimensions
///
/// This function ensures the TUI global state is initialized (if not already)
/// and renders the current page to a TestBackend, returning the rendered buffer as a string.
///
/// **Current Limitation**: Mock state synchronization is not yet implemented.
/// The renderer will render the default TUI state. For workflows that need specific state,
/// mock state manipulation must be implemented through direct TUI status writes using
/// `aoba::tui::status::write_status()`.
///
/// # Arguments
/// * `width` - Terminal width in characters
/// * `height` - Terminal height in characters
///
/// # Returns
/// A string representation of the rendered terminal buffer
pub fn render_tui_to_string(width: u16, height: u16) -> Result<String> {
    // Ensure global status is initialized
    // Note: If already initialized, this is a no-op due to init_status implementation
    ensure_status_initialized()?;

    // Synchronize mock JSON state into the live status tree so rendering reflects
    // the workflow-driven expectations in screen-capture mode.
    crate::mock_state::sync_mock_state_to_tui_status()?;

    // Create TestBackend
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;

    // Render the TUI
    terminal.draw(|frame| {
        // Call the main TUI render function
        if let Err(e) = aoba::tui::render_ui_for_testing(frame) {
            log::error!("Failed to render UI: {e}");
        }
    })?;

    // Get the buffer content as string
    let buffer = terminal.backend().buffer().clone();
    Ok(buffer_to_string(&buffer))
}

/// Ensure the TUI global status is initialized
fn ensure_status_initialized() -> Result<()> {
    use parking_lot::RwLock;
    use std::sync::Arc;

    // Try to read status - if it fails, status is not initialized
    let needs_init = aoba::tui::status::read_status(|_| Ok(())).is_err();

    if needs_init {
        // Initialize with default status
        let app = Arc::new(RwLock::new(aoba::tui::status::Status::default()));
        aoba::tui::status::init_status(app)?;
        log::debug!("Initialized TUI global status for testing");
    }

    Ok(())
}

/// Convert a ratatui Buffer to a string representation
fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area();
    let mut output = String::new();

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            output.push_str(cell.symbol());
        }
        if y < area.height - 1 {
            output.push('\n');
        }
    }

    output
}
