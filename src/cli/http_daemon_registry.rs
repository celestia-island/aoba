use anyhow::Result;
use futures::Future;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use flume::Sender;

type JoinOpt = Option<tokio::task::JoinHandle<Result<()>>>;

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
    handle: tokio::task::JoinHandle<Result<()>>,
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
pub async fn shutdown_and_join(port: u16) -> Result<()> {
    // send shutdown (best-effort)
    let maybe_entry = {
        let reg = HTTP_REGISTRY.lock().unwrap();
        reg.map.get(&port).map(|e| e.shutdown_tx.clone())
    };

    if let Some(tx) = maybe_entry {
        tx.send(()).unwrap_or(());
    }

    // take and join the handle if present
    let handle_to_await = {
        let mut reg = HTTP_REGISTRY.lock().unwrap();
        if let Some(entry) = reg.map.remove(&port) {
            entry.handle.lock().unwrap().take()
        } else {
            None
        }
    };

    if let Some(h) = handle_to_await {
        h.await.unwrap_or(Ok(()))?;
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
        std::mem::drop(shutdown_and_join(p));
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

/// Get the error result from a finished handle (non-blocking)
/// Returns Some(Err) if the handle has finished with an error, None if still running or not found
pub fn get_handle_error(port: u16) -> Option<Result<()>> {
    use std::task::Poll;

    let reg = HTTP_REGISTRY.lock().unwrap();
    if let Some(entry) = reg.map.get(&port) {
        let mut guard = entry.handle.lock().unwrap();
        if let Some(handle) = guard.as_mut() {
            // Create a no-op waker to poll the handle
            let waker = futures::task::noop_waker();
            let mut cx = std::task::Context::from_waker(&waker);

            // Try to poll the handle without awaiting
            match std::pin::Pin::new(handle).poll(&mut cx) {
                Poll::Ready(Ok(Ok(()))) => return Some(Ok(())),
                Poll::Ready(Ok(Err(e))) => {
                    log::error!("HTTP server thread error detected: {}", e);
                    return Some(Err(e));
                }
                Poll::Ready(Err(join_err)) => {
                    log::error!("HTTP server thread panicked: {}", join_err);
                    return Some(Err(anyhow::anyhow!(
                        "HTTP server thread panicked: {}",
                        join_err
                    )));
                }
                Poll::Pending => return None,
            }
        }
    }
    None
}
