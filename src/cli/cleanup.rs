use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

type Cleanup = Box<dyn FnOnce() + Send + 'static>;

/// Global cleanup registry: store cleanup closures and run them when requested.
#[derive(Default)]
struct CleanupRegistry {
    items: Vec<Cleanup>,
}

impl CleanupRegistry {
    fn register(&mut self, f: Cleanup) {
        self.items.push(f);
    }

    fn run_all(&mut self) {
        // Drain and run
        let items = std::mem::take(&mut self.items);
        for f in items {
            // Each cleanup gets its own catch_unwind to avoid aborting others
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                f();
            }));
        }
    }
}

static GLOBAL_CLEANUP: Lazy<Arc<Mutex<CleanupRegistry>>> =
    Lazy::new(|| Arc::new(Mutex::new(CleanupRegistry::default())));

fn global_registry() -> Arc<Mutex<CleanupRegistry>> {
    GLOBAL_CLEANUP.clone()
}

/// Register a cleanup closure to be run when `run_cleanups` is invoked.
pub fn register_cleanup(f: impl FnOnce() + Send + 'static) {
    let reg = global_registry();
    let mut guard = reg.lock().expect("cleanup registry lock");
    guard.register(Box::new(f));
}

/// Run all registered cleanup closures. Safe to call multiple times.
pub fn run_cleanups() {
    log::debug!("Running cleanup handlers...");
    let reg = global_registry();
    let mut guard = reg.lock().expect("cleanup registry lock");
    let count = guard.items.len();
    log::debug!("Running {} cleanup handlers", count);
    guard.run_all();
    log::debug!("Cleanup handlers completed");
}
