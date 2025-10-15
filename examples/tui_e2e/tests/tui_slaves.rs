use anyhow::{anyhow, Result};
use std::time::Duration;

use ci_utils::{
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ExpectKeyExt,
    ports::{port_exists, should_run_vcom_tests},
    snapshot::TerminalCapture,
    terminal::spawn_expect_process,
    tui::update_tui_registers,
};

use crate::utils::{configure_tui_slave_common, test_station_with_retries};

/// Test Multiple TUI Slaves on Single Port with IPC Communication
///
/// This test simulates 4 independent TUI slaves on vcom2 with different station IDs and register types:
/// - Slave 1: Station ID 1, Register Type 01 (Coil)
/// - Slave 2: Station ID 2, Register Type 02 (Discrete Input)
/// - Slave 3: Station ID 3, Register Type 03 (Holding Register)
/// - Slave 4: Station ID 4, Register Type 04 (Input Register)
///
/// Test Design:
/// - All slaves share the same vcom2 port but use different station IDs
/// - Uses IPC communication to avoid port conflicts
/// - Each slave has 12 registers with random data
/// - CLI masters on vcom1 poll each station to verify communication
///
/// The test validates:
/// 1. Multiple slaves can operate on the same port using different station IDs
/// 2. IPC communication prevents port conflicts
/// 3. Different register types work correctly
/// 4. Communication reliability with retry logic
pub async fn test_tui_slaves() -> Result<()> {
    const REGISTER_LENGTH: usize = 12;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Slaves test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Slaves E2E test");

    // Get port names from environment
    let port1 = std::env::var("AOBATEST_PORT1").unwrap_or_else(|_| "/tmp/vcom1".to_string());
    let port2 = std::env::var("AOBATEST_PORT2").unwrap_or_else(|_| "/tmp/vcom2".to_string());

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (CLI, polls all stations)");
    log::info!("  Slaves: {port2} (4 stations with different register types)");

    // Verify ports exist
    for (name, port) in [("port1", &port1), ("port2", &port2)] {
        if !port_exists(port) {
            return Err(anyhow!(
                "{name} ({port}) does not exist or is not available"
            ));
        }
    }
    log::info!("✅ Both virtual COM ports verified");

    // Spawn TUI process for slaves
    log::info!("🧪 Step 1: Spawning TUI Slaves process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI Slaves process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_seconds(3).await;

    // Configure all 4 slaves on vcom2
    let slaves = [
        (1, 1, "coil"),           // Station 1, Type 01 Coil
        (2, 2, "discrete_input"), // Station 2, Type 02 Discrete Input
        (3, 3, "holding"),        // Station 3, Type 03 Holding Register
        (4, 4, "input"),          // Station 4, Type 04 Input Register
    ];

    log::info!("🧪 Step 2: Configuring 4 slaves on {port2}");
    for &(station_id, register_type, register_mode) in &slaves {
        configure_tui_slave_common(
            &mut tui_session,
            &mut tui_cap,
            station_id,
            register_type,
            register_mode,
            REGISTER_LENGTH,
        )
        .await?;
    }

    // Generate and update register data for all slaves
    let slave_data: Vec<Vec<u16>> = (0..4)
        .map(|_| generate_random_registers(REGISTER_LENGTH))
        .collect();

    log::info!("🧪 Updating all slave registers");
    for (i, data) in slave_data.iter().enumerate() {
        log::info!("  Slave {} data: {:?}", i + 1, data);
        update_tui_registers(&mut tui_session, &mut tui_cap, data, false).await?;
    }

    // Wait for IPC updates to propagate
    log::info!("🧪 Waiting for IPC propagation...");
    tokio::time::sleep(Duration::from_millis(3000)).await;

    // Test all 4 stations from vcom1
    let mut station_success = std::collections::HashMap::new();

    for (i, &(station_id, _, register_mode)) in slaves.iter().enumerate() {
        log::info!("🧪 Testing Station {station_id} ({register_mode})");
        station_success.insert(
            station_id,
            test_station_with_retries(
                &port1,
                station_id,
                register_mode,
                &slave_data[i],
                MAX_RETRIES,
                RETRY_INTERVAL_MS,
            )
            .await?,
        );
    }

    // Check if all stations passed
    let all_passed = station_success.values().all(|&v| v);

    if all_passed {
        log::info!("✅ All stations passed!");
        for (station, success) in station_success.iter() {
            log::info!(
                "  Station {station}: {}",
                if *success { "✅ PASS" } else { "❌ FAIL" }
            );
        }
    } else {
        log::error!("❌ Some stations failed:");
        for (station, success) in station_success.iter() {
            log::error!(
                "  Station {station}: {}",
                if *success { "✅ PASS" } else { "❌ FAIL" }
            );
        }
        return Err(anyhow!("Not all stations passed the test"));
    }

    // Clean up TUI process
    log::info!("🧪 Cleaning up TUI process");
    tui_session.send_ctrl_c()?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!("✅ TUI Slaves test completed successfully!");
    Ok(())
}
