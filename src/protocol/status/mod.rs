pub mod types;

mod util;
pub use util::*;

pub mod debug_dump;

use anyhow::{anyhow, Result};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::protocol::status::types::Status;

/// Global status instance accessible via read_status and write_status
static STATUS: OnceCell<Arc<RwLock<Status>>> = OnceCell::new();

/// Initialize the global status instance. This should be called once at application startup.
pub fn init_status(status: Arc<RwLock<Status>>) -> Result<()> {
    STATUS
        .set(status)
        .map_err(|_| anyhow!("Status already initialized"))?;
    Ok(())
}

/// Experimental read-only accessor for `Status`.
///
/// - `f` is a user-provided closure that receives a reference to `Status` and
///   returns `Result<R, E>` (mapped to anyhow::Result here). The closure may
///   borrow from `Status`. The returned value will be cloned before leaving
///   the function to avoid lifetime issues. Therefore `R: Clone` is required.
pub fn read_status<R, F>(f: F) -> Result<R>
where
    F: FnOnce(&Status) -> Result<R>,
    R: Clone,
{
    let status = STATUS
        .get()
        .ok_or_else(|| anyhow!("Status not initialized"))?;
    let guard = status.read();
    // Call user closure with borrowed reference
    let val = f(&guard)?;
    // Clone once to decouple lifetime
    Ok(val.clone())
}

/// Experimental write accessor for `Status`.
///
/// - `f` is a FnMut that receives a mutable reference and may mutate status.
/// - The closure returns a `Result<R>`; the returned value will be cloned
///   before returning to avoid lifetime issues. Use `Ok(())` if no value is
///   needed.
pub fn write_status<R, F>(mut f: F) -> Result<R>
where
    F: FnMut(&mut Status) -> Result<R>,
    R: Clone,
{
    let status = STATUS
        .get()
        .ok_or_else(|| anyhow!("Status not initialized"))?;
    let mut guard = status.write();
    let val = f(&mut guard)?;
    Ok(val.clone())
}
