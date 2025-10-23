use std::time::Duration;

/// Small test helper to sleep for a short, fixed duration.
/// Kept here so examples and other helper modules can import it from the crate
/// root as `ci_utils::sleep_a_while`.
pub async fn sleep_a_while() {
    const DEFAULT_MS: u64 = 100; // Unified 100ms delay for all E2E tests
    tokio::time::sleep(Duration::from_millis(DEFAULT_MS)).await;
}

/// Sleep for an exact number of seconds (async-only helper used in tests)
///
/// Accepts an integer number of seconds and awaits that duration.
pub async fn sleep_seconds(secs: u64) {
    tokio::time::sleep(Duration::from_secs(secs)).await;
}
