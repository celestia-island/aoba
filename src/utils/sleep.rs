//! Sleep utilities

use std::time::Duration;

/// Sleep for 1 second (1000ms) - standard delay
pub async fn sleep_1s() {
    tokio::time::sleep(Duration::from_millis(1000)).await;
}

/// Sleep for 3 seconds (3000ms) - longer delay
pub async fn sleep_3s() {
    tokio::time::sleep(Duration::from_millis(3000)).await;
}

// Compatibility blocking helpers removed.
// Note: do not add blocking helpers here. Use `std::thread::sleep` only
// inside dedicated threads (spawned with `std::thread::spawn`).
