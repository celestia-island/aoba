pub use aoba_protocol::status::types::{CliMode, CliStatus, RegisterMode};

/// CLI status module
///
/// This module provides CLI-specific status structures for E2E testing.
/// Unlike TUI, CLI doesn't maintain a global status tree - it only has
/// serializable status for debug dumps.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// Manages periodic dumping of CLI status to a JSON file for E2E testing.
///
/// This struct is created when the `--debug-ci-e2e-test` flag is present.
/// It spawns a background task that periodically serializes the provided
/// status object and writes it to a file in `/tmp`.
pub struct CliStatusDumper {
    task_handle: tokio::task::JoinHandle<()>,
    should_stop: Arc<AtomicBool>,
}

impl CliStatusDumper {
    /// Create a new dumper and start the background task.
    pub fn new(status: CliStatus, port_name: &str) -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        let should_stop_clone = should_stop.clone();

        let port_basename = Path::new(port_name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown_port")
            .to_string();

        let task_handle = tokio::spawn(async move {
            let status_path = PathBuf::from(format!("/tmp/ci_cli_{port_basename}_status.json"));
            loop {
                if should_stop_clone.load(Ordering::Relaxed) {
                    break;
                }
                match status.to_json() {
                    Ok(json) => {
                        if let Err(e) = fs::write(&status_path, json) {
                            log::error!("Failed to write CLI status file: {e}");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize CLI status: {e}");
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });

        Self {
            task_handle,
            should_stop,
        }
    }

    /// Stop the background dumping task.
    pub async fn stop(self) {
        self.should_stop.store(true, Ordering::Relaxed);
        let _ = self.task_handle.await;
    }
}
