use std::time::Duration;

/// Small test helper to sleep for a short, fixed duration.
/// Kept here so examples and other helper modules can import it from the crate
/// root as `ci_utils::sleep_a_while`.
pub async fn sleep_a_while() {
    const DEFAULT_MS: u64 = 500;
    tokio::time::sleep(Duration::from_millis(DEFAULT_MS)).await;
}
