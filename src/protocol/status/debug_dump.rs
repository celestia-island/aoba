use anyhow::Result;

/// Debug dump utilities for CI/E2E testing
///
/// This module provides functionality to periodically dump the global status tree
/// to a file for external monitoring during E2E tests.
use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

/// Flag to control whether debug dumping is enabled
static DEBUG_DUMP_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable debug dump mode (should be called when --debug-ci-e2e-test is set)
pub fn enable_debug_dump() {
    DEBUG_DUMP_ENABLED.store(true, Ordering::SeqCst);
}

/// Check if debug dump mode is enabled
pub fn is_debug_dump_enabled() -> bool {
    DEBUG_DUMP_ENABLED.load(Ordering::SeqCst)
}

/// Start a background thread that periodically dumps status to a file
///
/// The file is overwritten (not appended) on each dump to keep file size manageable.
/// Dumps occur every 500ms.
///
/// # Arguments
/// * `output_path` - Path to the output file (e.g., "/tmp/ci_tui_status.json" or "/tmp/ci_cli_vcom1_status.json")
/// * `shutdown_signal` - Optional Arc<AtomicBool> to signal thread shutdown
/// * `status_fn` - Function to call to get the status to dump
///
/// # Returns
/// A JoinHandle to the spawned thread
pub fn start_status_dump_thread<F>(
    output_path: PathBuf,
    shutdown_signal: Option<Arc<AtomicBool>>,
    status_fn: F,
) -> thread::JoinHandle<()>
where
    F: Fn() -> Result<String> + Send + 'static,
{
    thread::spawn(move || {
        log::info!(
            "Started status dump thread, writing to {}",
            output_path.display()
        );

        loop {
            // Check shutdown signal
            if let Some(ref signal) = shutdown_signal {
                if signal.load(Ordering::SeqCst) {
                    log::info!("Status dump thread shutting down");
                    break;
                }
            }

            // Dump status to file
            if let Err(e) = dump_status_with_fn(&output_path, &status_fn) {
                log::warn!("Failed to dump status to {}: {}", output_path.display(), e);
            }

            // Sleep for 500ms
            thread::sleep(Duration::from_millis(500));
        }
    })
}

/// Dump status using the provided function (overwrites existing content)
fn dump_status_with_fn<F>(path: &PathBuf, status_fn: &F) -> Result<()>
where
    F: Fn() -> Result<String>,
{
    let json = status_fn()?;

    // Write to file (overwrite mode)
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    file.flush()?;

    Ok(())
}
