use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::utils::{configure_tui_master_common, setup_tui_port, test_station_with_retries};
use ci_utils::{
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ExpectKeyExt,
    ports::{port_exists, should_run_vcom_tests},
    snapshot::TerminalCapture,
    terminal::spawn_expect_process,
    tui::update_tui_registers,
};

/// Test Multiple TUI Masters on Single Port with Different Register Types
///
/// This test simulates 4 TUI masters on vcom1 with different station IDs and different register types:
/// - Master 1: Station ID 1, Register Type 03 (Holding Register)
/// - Master 2: Station ID 2, Register Type 04 (Input Register)
/// - Master 3: Station ID 3, Register Type 01 (Coils)
/// - Master 4: Station ID 4, Register Type 02 (Discrete Inputs)
///
/// Test Design:
/// - All masters share the same vcom1 port with different station IDs and register types
/// - Uses IPC communication to avoid port conflicts
/// - Each master has 6 registers with random data
/// - CLI slaves on vcom2 poll each station to verify communication
///
/// The test validates:
/// 1. Multiple masters can operate on the same port with different station IDs and register types
/// 2. IPC communication prevents port conflicts
/// 3. All register types work correctly (Holding, Input, Coils, Discrete Inputs)
/// 4. Communication reliability with retry logic
pub async fn test_tui_multi_masters_different_registers() -> Result<()> {
    const REGISTER_LENGTH: usize = 6;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Multi-Masters Different Registers test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Masters Different Registers E2E test");

    // Get platform-appropriate port names from ci_utils (handles env overrides and defaults)
    let ports = ci_utils::ports::vcom_matchers();
    let port1 = ports.port1_name.clone();
    let port2 = ports.port2_name.clone();

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (4 stations with different station IDs and register types)");
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

    // Configure 4 masters on vcom1 with different station IDs and register types
    let masters = [
        (1, 3, "holding"),  // Station 1, Type 03 Holding Register
        (2, 4, "input"),    // Station 2, Type 04 Input Register
        (3, 1, "coils"),    // Station 3, Type 01 Coils
        (4, 2, "discrete"), // Station 4, Type 02 Discrete Inputs
    ];

    log::info!("🧪 Step 2: Configuring 4 masters on {port1} with different register types");
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
        let (station_id, register_type, register_mode) = masters[i];
        log::info!(
            "  Master {} (Station {station_id}, Type {register_type}, {register_mode}) data: {data:?}",
            i + 1
        );
        update_tui_registers(&mut tui_session, &mut tui_cap, data, false).await?;
    }

    // Wait for IPC updates to propagate
    log::info!("🧪 Waiting for IPC propagation...");
    tokio::time::sleep(Duration::from_millis(3000)).await;

    // Test all 4 stations from vcom2
    let mut station_success = std::collections::HashMap::new();

    for (i, &(station_id, register_type, register_mode)) in masters.iter().enumerate() {
        log::info!("🧪 Testing Station {station_id} (Type {register_type}, {register_mode})");
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

    log::info!("✅ TUI Multi-Masters Different Registers test completed successfully!");
    Ok(())
}
