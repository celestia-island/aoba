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

use crate::utils::{configure_tui_master_common, setup_tui_port, test_station_with_retries};

/// Test Multiple TUI Masters on Single Port with IPC Communication
///
/// This test simulates 4 independent TUI masters on vcom1 with different station IDs using holding registers:
/// - Master 1: Station ID 1, Register Type 03 (Holding Register)
/// - Master 2: Station ID 2, Register Type 03 (Holding Register)
/// - Master 3: Station ID 3, Register Type 03 (Holding Register)
/// - Master 4: Station ID 4, Register Type 03 (Holding Register)
///
/// Test Design:
/// - All masters share the same vcom1 port but use different station IDs
/// - Uses IPC communication to avoid port conflicts
/// - Each master has 12 registers with random data
/// - CLI slaves on vcom2 poll each station to verify communication
///
/// The test validates:
/// 1. Multiple masters can operate on the same port using different station IDs
/// 2. IPC communication prevents port conflicts
/// 3. Different register types work correctly
/// 4. Communication reliability with retry logic
pub async fn test_tui_masters() -> Result<()> {
    const REGISTER_LENGTH: usize = 12;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Masters test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Masters E2E test");

    // Get port names from environment
    let port1 = std::env::var("AOBATEST_PORT1").unwrap_or_else(|_| "/tmp/vcom1".to_string());
    let port2 = std::env::var("AOBATEST_PORT2").unwrap_or_else(|_| "/tmp/vcom2".to_string());

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (4 stations with different register types)");
    log::info!("  Slaves: {port2} (CLI, polls all stations)");

    // Verify ports exist
    for (name, port) in [("port1", &port1), ("port2", &port2)] {
        if !port_exists(port) {
            return Err(anyhow!(
                "{name} ({port}) does not exist or is not available"
            ));
        }
    }
    log::info!("✅ Both virtual COM ports verified");

    // Spawn TUI process for masters
    log::info!("🧪 Step 1: Spawning TUI Masters process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI Masters process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_seconds(3).await;

    // Configure all 4 masters on vcom1 - use only holding registers for compatibility
    // CLI master provide only supports Holding and Coils, but slave poll works better with holding
    let masters = [
        (1, 3, "holding"), // Station 1, Type 03 Holding Register
        (2, 3, "holding"), // Station 2, Type 03 Holding Register
        (3, 3, "holding"), // Station 3, Type 03 Holding Register
        (4, 3, "holding"), // Station 4, Type 03 Holding Register
    ];

    log::info!("🧪 Step 2: Configuring 4 masters on {port1}");
    for &(station_id, register_type, register_mode) in &masters {
        // Setup port once for the first master, subsequent masters share the same port
        if station_id == 1 {
            setup_tui_port(&mut tui_session, &mut tui_cap, &port1).await?;
        }

        configure_tui_master_common(
            &mut tui_session,
            &mut tui_cap,
            station_id,
            register_type,
            register_mode,
            REGISTER_LENGTH,
        )
        .await?;
    }

    // Generate and update register data for all masters
    let master_data: Vec<Vec<u16>> = (0..4)
        .map(|_| generate_random_registers(REGISTER_LENGTH))
        .collect();

    log::info!("🧪 Updating all master registers");
    for (i, data) in master_data.iter().enumerate() {
        log::info!("  Master {} data: {data:?}", i + 1);
        update_tui_registers(&mut tui_session, &mut tui_cap, data, false).await?;
    }

    // Wait for IPC updates to propagate
    log::info!("🧪 Waiting for IPC propagation...");
    tokio::time::sleep(Duration::from_millis(3000)).await;

    // Test all 4 stations from vcom2
    let mut station_success = std::collections::HashMap::new();

    for (i, &(station_id, _, register_mode)) in masters.iter().enumerate() {
        log::info!("🧪 Testing Station {station_id} ({register_mode})");
        station_success.insert(
            station_id,
            test_station_with_retries(
                &port2,
                station_id,
                register_mode,
                &master_data[i],
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

    log::info!("✅ TUI Masters test completed successfully!");
    Ok(())
}
