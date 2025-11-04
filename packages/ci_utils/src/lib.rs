//! CI utilities for E2E testing
//!
//! This package provides shared utilities for testing TUI and CLI components,
//! including IPC message definitions and test helpers.

pub mod ipc;
pub mod ipc_messages;

pub use ipc::{IpcReceiver, IpcSender};
pub use ipc_messages::*;
