pub mod types;

mod util;
pub use util::*;

pub mod debug_dump;

use anyhow::{anyhow, Result};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;

/// Generic read-only accessor for status with an explicit static reference.
///
/// - `status_cell` is the OnceCell containing the status
/// - `f` is a user-provided closure that receives a reference to the status and
///   returns `Result<R, E>` (mapped to anyhow::Result here). The closure may
///   borrow from status. The returned value will be cloned before leaving
///   the function to avoid lifetime issues. Therefore `R: Clone` is required.
pub fn read_status_generic<S, R, F>(status_cell: &OnceCell<Arc<RwLock<S>>>, f: F) -> Result<R>
where
    F: FnOnce(&S) -> Result<R>,
    R: Clone,
{
    let status = status_cell
        .get()
        .ok_or_else(|| anyhow!("Status not initialized"))?;
    let guard = status.read();
    // Call user closure with borrowed reference
    let val = f(&guard)?;
    // Clone once to decouple lifetime
    Ok(val.clone())
}

/// Generic write accessor for status with an explicit static reference.
///
/// - `status_cell` is the OnceCell containing the status
/// - `f` is a FnMut that receives a mutable reference and may mutate status.
/// - The closure returns a `Result<R>`; the returned value will be cloned
///   before returning to avoid lifetime issues. Use `Ok(())` if no value is
///   needed.
pub fn write_status_generic<S, R, F>(status_cell: &OnceCell<Arc<RwLock<S>>>, mut f: F) -> Result<R>
where
    F: FnMut(&mut S) -> Result<R>,
    R: Clone,
{
    let status = status_cell
        .get()
        .ok_or_else(|| anyhow!("Status not initialized"))?;
    let mut guard = status.write();
    let val = f(&mut guard)?;
    Ok(val.clone())
}

/// Generic initialization function for status.
pub fn init_status_generic<S>(
    status_cell: &OnceCell<Arc<RwLock<S>>>,
    status: Arc<RwLock<S>>,
) -> Result<()> {
    status_cell
        .set(status)
        .map_err(|_| anyhow!("Status already initialized"))?;
    Ok(())
}
