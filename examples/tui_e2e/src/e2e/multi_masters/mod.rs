// TUI Multi-masters test modules
pub mod basic;
pub mod different_registers;
pub mod same_station;

pub use basic::test_tui_multi_masters_basic;
pub use different_registers::test_tui_multi_masters_different_registers;
pub use same_station::test_tui_multi_masters_same_station;

/// Main entry point for TUI multi-masters tests
pub async fn test_tui_multi_masters(port1: &str, port2: &str) -> anyhow::Result<()> {
    log::info!("🧪 Starting TUI Multi-Masters E2E test suite");

    // Test 1: Basic multi-masters with different station IDs
    log::info!("🧪 Test 1/3: Basic multi-masters with different station IDs");
    test_tui_multi_masters_basic(port1, port2).await?;

    // Wait for cleanup to complete and port to be fully released
    log::info!("⏱️ Waiting for port cleanup between tests...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Test 2: Multi-masters with same station ID but different register types
    log::info!("🧪 Test 2/3: Multi-masters with same station ID but different register types");
    test_tui_multi_masters_same_station(port1, port2).await?;

    // Wait for cleanup to complete and port to be fully released
    log::info!("⏱️ Waiting for port cleanup between tests...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Test 3: Multi-masters with different register types
    log::info!("🧪 Test 3/3: Multi-masters with different register types");
    test_tui_multi_masters_different_registers(port1, port2).await?;

    log::info!("✅ TUI Multi-Masters E2E test suite completed successfully");
    Ok(())
}
