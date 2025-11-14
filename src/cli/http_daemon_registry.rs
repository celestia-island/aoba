use anyhow::Result;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use flume::Sender;

type JoinOpt = Option<std::thread::JoinHandle<()>>;

struct Entry {
    handle: Arc<Mutex<JoinOpt>>,
    shutdown_tx: Sender<()>,
}

/// Registry mapping port -> Entry(handle, shutdown sender).
#[derive(Default)]
struct Registry {
    map: HashMap<u16, Entry>,
}

static HTTP_REGISTRY: Lazy<Mutex<Registry>> = Lazy::new(|| Mutex::new(Registry::default()));

/// Insert a handle and shutdown sender for given port.
pub fn register_handle(
    port: u16,
    handle: std::thread::JoinHandle<()>,
    shutdown_tx: Sender<()>,
) -> Arc<Mutex<JoinOpt>> {
    let arc = Arc::new(Mutex::new(Some(handle)));
    let entry = Entry {
        handle: arc.clone(),
        shutdown_tx,
    };
    let mut reg = HTTP_REGISTRY.lock().unwrap();
    reg.map.insert(port, entry);
    arc
}

/// Attempt to shutdown the HTTP daemon by sending shutdown on channel and join the thread.
pub fn shutdown_and_join(port: u16) -> Result<()> {
    // send shutdown (best-effort)
    let maybe_entry = {
        let reg = HTTP_REGISTRY.lock().unwrap();
        reg.map.get(&port).map(|e| e.shutdown_tx.clone())
    };

    if let Some(tx) = maybe_entry {
        let _ = tx.send(());
    }

    // take and join the handle if present
    let maybe_entry = {
        let mut reg = HTTP_REGISTRY.lock().unwrap();
        reg.map.remove(&port)
    };

    if let Some(entry) = maybe_entry {
        if let Some(h) = entry.handle.lock().unwrap().take() {
            let _ = h.join();
        }
    }

    Ok(())
}

/// Shutdown all registered HTTP daemons and join them.
pub fn shutdown_all_and_join() {
    let ports: Vec<u16> = {
        let reg = HTTP_REGISTRY.lock().unwrap();
        reg.map.keys().copied().collect()
    };

    for p in ports {
        let _ = shutdown_and_join(p);
    }
}

/// Check whether a registered handle for the given port is finished.
/// Returns None if no handle registered for port, Some(true) if finished.
pub fn is_handle_finished(port: u16) -> Option<bool> {
    let reg = HTTP_REGISTRY.lock().unwrap();
    if let Some(entry) = reg.map.get(&port) {
        let guard = entry.handle.lock().unwrap();
        return Some(guard.as_ref().map(|h| h.is_finished()).unwrap_or(true));
    }
    None
}
