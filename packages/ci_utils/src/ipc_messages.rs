//! Shared IPC message types for TUI E2E testing
//!
//! This module defines the message protocol used for IPC communication between
//! TUI E2E tests and the TUI process during testing.

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

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
    ScreenContent {
        content: String,
        width: u16,
        height: u16,
    },
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
    /// Create IPC socket names for the given ID
    /// Uses platform-appropriate naming (paths on Unix, namespaced on Linux with abstract sockets)
    pub fn socket_names(&self) -> (String, String) {
        let to_tui = format!("aoba_e2e_to_tui_{}", self.0);
        let from_tui = format!("aoba_tui_to_e2e_{}", self.0);
        (to_tui, from_tui)
    }

    /// Legacy method for compatibility - returns paths
    /// Note: The actual socket implementation will use platform-appropriate naming
    #[deprecated(note = "Use socket_names() instead")]
    pub fn paths(&self) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir();
        let to_tui = base.join(format!("aoba_e2e_to_tui_{}.sock", self.0));
        let from_tui = base.join(format!("aoba_tui_to_e2e_{}.sock", self.0));
        (to_tui, from_tui)
    }
}

/// Connection timeout while waiting for sockets to appear
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Interval between connection retry attempts
pub const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(100);
/// Timeout for a single read/write operation
pub const IO_TIMEOUT: Duration = Duration::from_secs(30);
