/// TUI status module
///
/// This module provides the TUI-specific status tree and read/write functions.

pub mod global;
pub mod serializable;

// Re-export main types
pub use global::{ErrorInfo, Page, Status};
pub use serializable::TuiStatus;

use anyhow::Result;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;

/// Global TUI status instance
static TUI_STATUS: OnceCell<Arc<RwLock<Status>>> = OnceCell::new();

/// Initialize the TUI status instance. This should be called once at application startup.
pub fn init_status(status: Arc<RwLock<Status>>) -> Result<()> {
    crate::protocol::status::init_status_generic(&TUI_STATUS, status)
}

/// TUI-specific read-only accessor for `Status`.
///
/// This is a wrapper around the generic read_status function that uses the TUI status tree.
pub fn read_status<R, F>(f: F) -> Result<R>
where
    F: FnOnce(&Status) -> Result<R>,
    R: Clone,
{
    crate::protocol::status::read_status_generic(&TUI_STATUS, f)
}

/// TUI-specific write accessor for `Status`.
///
/// This is a wrapper around the generic write_status function that uses the TUI status tree.
pub fn write_status<R, F>(f: F) -> Result<R>
where
    F: FnMut(&mut Status) -> Result<R>,
    R: Clone,
{
    crate::protocol::status::write_status_generic(&TUI_STATUS, f)
}
