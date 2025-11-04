//! TUI renderer for TestBackend
//!
//! This module provides utilities to render the TUI application to a `TestBackend`
//! for snapshot testing, without needing to spawn a real process.

use anyhow::Result;
use ratatui::{backend::TestBackend, Terminal};

/// Render the TUI to a TestBackend with specified dimensions
///
/// This function ensures the TUI global state is initialized (if not already)
/// and renders the current page to a TestBackend, returning the rendered buffer as a string.
///
/// Note: Currently, mock state synchronization is not fully implemented.
/// The renderer will render the default TUI state. Mock state manipulation
/// will need to be implemented through direct TUI status writes.
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

    // TODO: Implement mock state synchronization
    // For now, we render whatever is in the TUI global status
    // 
    // To support workflows that require specific state:
    // 1. Parse mock_state JSON to TuiStatus structure
    // 2. Convert TuiStatus to internal Status representation
    // 3. Apply to global status via write_status()
    // 
    // GitHub Issue: Track this as a separate enhancement if workflows require it

    // Create TestBackend
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;

    // Render the TUI
    terminal.draw(|frame| {
        // Call the main TUI render function
        if let Err(e) = aoba::tui::render_ui_for_testing(frame) {
            log::error!("Failed to render UI: {}", e);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_basic() {
        // Basic smoke test to ensure rendering doesn't panic
        let result = render_tui_to_string(80, 24);
        assert!(result.is_ok());
    }
}
