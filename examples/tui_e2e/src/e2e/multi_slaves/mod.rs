// TUI Multi-slaves test modules
pub mod adjacent_registers;
pub mod basic;
pub mod same_station;

pub use adjacent_registers::test_tui_multi_slaves_adjacent_registers;
pub use basic::test_tui_multi_slaves_basic;
pub use same_station::test_tui_multi_slaves_same_station;

/// Main entry point for TUI multi-slaves tests
pub async fn test_tui_multi_slaves(port1: &str, port2: &str) -> anyhow::Result<()> {
    log::info!("🧪 Starting TUI Multi-Slaves E2E test suite");

    // Test 1: Basic multi-slaves with different station IDs
    log::info!("🧪 Test 1/3: Basic multi-slaves with different station IDs");
    crate::cleanup_tui_config_cache()?;
    test_tui_multi_slaves_basic(port1, port2).await?;

    // Wait for cleanup to complete and port to be fully released
    log::info!("⏱️ Waiting for port cleanup between tests...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Test 2: Multi-slaves with same station ID but different register types
    log::info!("🧪 Test 2/3: Multi-slaves with same station ID but different register types");
    crate::cleanup_tui_config_cache()?;
    test_tui_multi_slaves_same_station(port1, port2).await?;

    // Wait for cleanup to complete and port to be fully released
    log::info!("⏱️ Waiting for port cleanup between tests...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Test 3: Multi-slaves with adjacent registers
    log::info!("🧪 Test 3/3: Multi-slaves with adjacent registers");
    crate::cleanup_tui_config_cache()?;
    test_tui_multi_slaves_adjacent_registers(port1, port2).await?;

    log::info!("✅ TUI Multi-Slaves E2E test suite completed successfully");
    Ok(())
}
