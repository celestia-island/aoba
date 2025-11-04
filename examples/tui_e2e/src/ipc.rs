//! IPC communication for TUI E2E testing
//!
//! This module provides IPC-based communication between the TUI process and E2E tests.
//! Instead of using expectrl/vt100 to capture terminal output, we use a direct IPC channel
//! where:
//! - E2E tests send keyboard events to TUI
//! - TUI sends rendered screen content back to E2E tests

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Message from E2E test to TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum E2EToTuiMessage {
    /// Simulate a key press
    KeyPress { key: String },
    /// Simulate character input (typing)
    CharInput { ch: char },
    /// Request current screen rendering
    RequestScreen,
    /// Shutdown the TUI
    Shutdown,
}

/// Message from TUI to E2E test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TuiToE2EMessage {
    /// Screen content as rendered text
    ScreenContent { content: String, width: u16, height: u16 },
    /// Acknowledgment of key press
    KeyProcessed,
    /// TUI is ready
    Ready,
    /// Error occurred
    Error { message: String },
}

/// IPC channel identifier
#[derive(Debug, Clone)]
pub struct IpcChannelId(pub String);

impl IpcChannelId {
    /// Create IPC channel paths for the given ID
    pub fn paths(&self) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir();
        let to_tui = base.join(format!("aoba_e2e_to_tui_{}.sock", self.0));
        let from_tui = base.join(format!("aoba_tui_to_e2e_{}.sock", self.0));
        (to_tui, from_tui)
    }
}

/// IPC sender (E2E test side)
pub struct IpcSender {
    channel_id: IpcChannelId,
    // TODO: Add actual IPC implementation (Unix domain socket or named pipe)
}

impl IpcSender {
    /// Create a new IPC sender
    pub fn new(channel_id: IpcChannelId) -> Result<Self> {
        Ok(Self { channel_id })
    }

    /// Send a message to TUI
    pub fn send(&mut self, message: E2EToTuiMessage) -> Result<()> {
        log::debug!("IPC Send: {:?}", message);
        // TODO: Implement actual IPC send
        Ok(())
    }

    /// Receive a message from TUI
    pub fn receive(&mut self) -> Result<TuiToE2EMessage> {
        log::debug!("IPC Receive: waiting for message from TUI");
        // TODO: Implement actual IPC receive
        Ok(TuiToE2EMessage::Ready)
    }

    /// Send key press and wait for screen update
    pub fn send_key_and_get_screen(&mut self, key: String) -> Result<String> {
        self.send(E2EToTuiMessage::KeyPress { key })?;
        self.send(E2EToTuiMessage::RequestScreen)?;
        
        match self.receive()? {
            TuiToE2EMessage::ScreenContent { content, .. } => Ok(content),
            msg => anyhow::bail!("Unexpected message: {:?}", msg),
        }
    }
}

/// IPC receiver (TUI side)
pub struct IpcReceiver {
    channel_id: IpcChannelId,
    // TODO: Add actual IPC implementation
}

impl IpcReceiver {
    /// Create a new IPC receiver
    pub fn new(channel_id: IpcChannelId) -> Result<Self> {
        Ok(Self { channel_id })
    }

    /// Receive a message from E2E test
    pub fn receive(&mut self) -> Result<E2EToTuiMessage> {
        log::debug!("IPC Receive: waiting for message from E2E test");
        // TODO: Implement actual IPC receive
        Ok(E2EToTuiMessage::RequestScreen)
    }

    /// Send a message to E2E test
    pub fn send(&mut self, message: TuiToE2EMessage) -> Result<()> {
        log::debug!("IPC Send: {:?}", message);
        // TODO: Implement actual IPC send
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_paths() {
        let id = IpcChannelId("test123".to_string());
        let (to_tui, from_tui) = id.paths();
        
        assert!(to_tui.to_string_lossy().contains("aoba_e2e_to_tui_test123"));
        assert!(from_tui.to_string_lossy().contains("aoba_tui_to_e2e_test123"));
    }
}
