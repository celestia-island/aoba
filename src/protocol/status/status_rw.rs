use anyhow::{anyhow, Result};
use std::sync::{Arc, RwLock};

use crate::protocol::status::Status;

/// Experimental read-only accessor for `Status`.
///
/// - `s` is the shared `Arc<RwLock<Status>>` used across the app.
/// - `f` is a user-provided closure that receives a reference to `Status` and
///   returns `Result<R, E>` (mapped to anyhow::Result here). The closure may
///   borrow from `Status`. The returned value will be cloned before leaving
///   the function to avoid lifetime issues. Therefore `R: Clone` is required.
pub fn read_status<R, F>(s: &Arc<RwLock<Status>>, f: F) -> Result<R>
where
    F: FnOnce(&Status) -> Result<R>,
    R: Clone,
{
    let guard = s
        .read()
        .map_err(|err| anyhow!("status lock poisoned: {}", err))?;
    // Call user closure with borrowed reference
    let val = f(&*guard)?;
    // Clone once to decouple lifetime
    Ok(val.clone())
}

/// Experimental write accessor for `Status`.
///
/// - `f` is a FnMut that receives a mutable reference and may mutate status.
/// - The closure returns a `Result<R>`; the returned value will be cloned
///   before returning to avoid lifetime issues. Use `Ok(())` if no value is
///   needed.
pub fn write_status<R, F>(s: &Arc<RwLock<Status>>, mut f: F) -> Result<R>
where
    F: FnMut(&mut Status) -> Result<R>,
    R: Clone,
{
    let mut guard = s
        .write()
        .map_err(|err| anyhow!("status lock poisoned: {}", err))?;
    let val = f(&mut *guard)?;
    Ok(val.clone())
}
