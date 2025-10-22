/// Helper module for sending semantic key inputs to TUI sessions
///
/// This module provides a clean API for sending keyboard input to TUI applications
/// using semantic names instead of hardcoded escape sequences.
use anyhow::{anyhow, Result};

use expectrl::Expect;

/// Extension trait for expectrl::Expect to add semantic key sending methods
pub trait ExpectKeyExt {
    /// Send an arrow key
    fn send_arrow(&mut self, direction: ArrowKey) -> Result<()>;

    /// Send Enter key
    fn send_enter(&mut self) -> Result<()>;

    /// Send Tab key
    fn send_tab(&mut self) -> Result<()>;

    /// Send Escape key
    fn send_escape(&mut self) -> Result<()>;

    /// Send Ctrl+C to exit program quickly
    fn send_ctrl_c(&mut self) -> Result<()>;

    /// Send Ctrl+S to save
    fn send_ctrl_s(&mut self) -> Result<()>;

    /// Send Ctrl+A to select all
    fn send_ctrl_a(&mut self) -> Result<()>;

    /// Send Backspace key
    fn send_backspace(&mut self) -> Result<()>;

    /// Send PageUp key
    fn send_page_up(&mut self) -> Result<()>;

    /// Send PageDown key
    fn send_page_down(&mut self) -> Result<()>;

    /// Send Ctrl+PageUp key
    fn send_ctrl_page_up(&mut self) -> Result<()>;

    /// Send Ctrl+PageDown key
    fn send_ctrl_page_down(&mut self) -> Result<()>;

    /// Send a character key
    fn send_char(&mut self, ch: char) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub enum ArrowKey {
    Up,
    Down,
    Left,
    Right,
}

impl<T: Expect> ExpectKeyExt for T {
    fn send_arrow(&mut self, direction: ArrowKey) -> Result<()> {
        let seq = match direction {
            ArrowKey::Up => "\x1b[A",
            ArrowKey::Down => "\x1b[B",
            ArrowKey::Right => "\x1b[C",
            ArrowKey::Left => "\x1b[D",
        };
        self.send(seq)
            .map_err(|err| anyhow!("Failed to send {direction:?} arrow: {err}"))
    }

    fn send_enter(&mut self) -> Result<()> {
        self.send("\r")
            .map_err(|err| anyhow!("Failed to send Enter: {err}"))
    }

    fn send_tab(&mut self) -> Result<()> {
        self.send("\t")
            .map_err(|err| anyhow!("Failed to send Tab: {err}"))
    }

    fn send_escape(&mut self) -> Result<()> {
        self.send("\x1b")
            .map_err(|err| anyhow!("Failed to send Escape: {err}"))
    }

    fn send_ctrl_c(&mut self) -> Result<()> {
        self.send("\x03")
            .map_err(|err| anyhow!("Failed to send Ctrl+C: {err}"))
    }

    fn send_ctrl_s(&mut self) -> Result<()> {
        self.send("\x13")
            .map_err(|err| anyhow!("Failed to send Ctrl+S: {err}"))
    }

    fn send_ctrl_a(&mut self) -> Result<()> {
        self.send("\x01")
            .map_err(|err| anyhow!("Failed to send Ctrl+A: {err}"))
    }

    fn send_backspace(&mut self) -> Result<()> {
        self.send("\x7f")
            .map_err(|err| anyhow!("Failed to send Backspace: {err}"))
    }

    fn send_page_up(&mut self) -> Result<()> {
        // PageUp escape sequence
        self.send("\x1b[5~")
            .map_err(|err| anyhow!("Failed to send PageUp: {err}"))
    }

    fn send_page_down(&mut self) -> Result<()> {
        // PageDown escape sequence
        self.send("\x1b[6~")
            .map_err(|err| anyhow!("Failed to send PageDown: {err}"))
    }

    fn send_ctrl_page_up(&mut self) -> Result<()> {
        // Ctrl+PageUp escape sequence
        self.send("\x1b[5;5~")
            .map_err(|err| anyhow!("Failed to send Ctrl+PageUp: {err}"))
    }

    fn send_ctrl_page_down(&mut self) -> Result<()> {
        // Ctrl+PageDown escape sequence
        self.send("\x1b[6;5~")
            .map_err(|err| anyhow!("Failed to send Ctrl+PageDown: {err}"))
    }

    fn send_char(&mut self, ch: char) -> Result<()> {
        let s = ch.to_string();
        self.send(&s)
            .map_err(|err| anyhow!("Failed to send character '{ch}': {err}"))
    }
}

/// Convenience functions for common key sequences
/// Send multiple arrow keys in sequence
pub fn send_arrows<T: Expect>(session: &mut T, direction: ArrowKey, count: usize) -> Result<()> {
    for _ in 0..count {
        session.send_arrow(direction)?;
    }
    Ok(())
}
