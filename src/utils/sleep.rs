//! Sleep utilities for E2E testing
//!
//! This module provides standardized sleep functions for use in E2E tests
//! to replace direct `tokio::time::sleep` calls with specific durations.

use std::time::Duration;

/// Sleep for 1 second (1000ms) - standard delay for CI/E2E tests
pub async fn sleep_1s() {
    tokio::time::sleep(Duration::from_millis(1000)).await;
}

/// Sleep for 3 seconds (3000ms) - longer delay for CI/E2E tests
pub async fn sleep_3s() {
    tokio::time::sleep(Duration::from_millis(3000)).await;
}
