use std::time::Duration;

/// Sleep for 1 second (1000ms) - standard delay for CI/E2E tests
pub async fn sleep_1s() {
    tokio::time::sleep(Duration::from_millis(1000)).await;
}

/// Sleep for 3 seconds (3000ms) - extended delay for complex operations
pub async fn sleep_3s() {
    tokio::time::sleep(Duration::from_millis(3000)).await;
}

/// Sleep for an exact number of seconds (async-only helper used in tests)
///
/// Accepts an integer number of seconds and awaits that duration.
pub async fn sleep_seconds(secs: u64) {
    tokio::time::sleep(Duration::from_secs(secs)).await;
}
