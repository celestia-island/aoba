//! Shared utilities for Aoba
//!
//! This package provides general-purpose utilities used across the application:
//! - Internationalization (i18n) support
//! - IPC communication helpers for testing
//! - Message type definitions

pub mod i18n;
pub mod ipc;
pub mod ipc_messages;

pub use ipc::{IpcReceiver, IpcSender};
pub use ipc_messages::*;
