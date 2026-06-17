//! Shared utilities for Aoba
//!
//! This package provides general-purpose utilities used across the application:
//! - Internationalization (i18n) support
//! - IPC communication helpers for testing
//! - Message type definitions

pub mod form;
pub mod i18n;
pub mod ipc;
pub mod ipc_messages;
pub mod ports;
pub mod sleep;

pub use form::*;
pub use ipc::{IpcReceiver, IpcSender};
pub use ipc_messages::*;
pub use ports::*;
pub use sleep::*;

/// Convert a byte slice into an uppercase hexadecimal string separated by spaces.
#[must_use]
pub fn format_hex_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
