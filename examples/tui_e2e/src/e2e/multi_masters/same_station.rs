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

/// Test Multiple TUI Masters on Single Port with Same Station ID but Different Register Types
///
/// This test simulates 3 TUI masters on vcom1 with the same station ID but different register types:
/// - Master 1: Station ID 1, Register Type 03 (Holding Register)
/// - Master 2: Station ID 1, Register Type 04 (Input Register)
/// - Master 3: Station ID 1, Register Type 01 (Coils)
///
/// Test Design:
/// - All masters share the same vcom1 port and same station ID but different register types
/// - Uses IPC communication to avoid port conflicts
/// - Each master has 8 registers with random data
/// - CLI slaves on vcom2 poll each register type to verify communication
///
/// The test validates:
/// 1. Multiple masters can operate on the same port with same station ID but different register types
/// 2. IPC communication prevents port conflicts
/// 3. Different register types work correctly within the same station
/// 4. Communication reliability with retry logic
pub async fn test_tui_multi_masters_same_station() -> Result<()> {
    const REGISTER_LENGTH: usize = 8;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Multi-Masters Same Station test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Masters Same Station E2E test");

    // Get platform-appropriate port names from ci_utils (handles env overrides and defaults)
    let ports = ci_utils::ports::vcom_matchers();
    let port1 = ports.port1_name.clone();
    let port2 = ports.port2_name.clone();

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (3 masters with same station ID, different register types)");
    log::info!("  Slaves: {port2} (CLI, polls all register types)");

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

    // Configure 3 masters on vcom1 with same station ID but different register types
    let masters = [
        (1, 3, "holding", 0), // Station 1, Type 03 Holding Register, Address 0
        (1, 4, "input", 0),   // Station 1, Type 04 Input Register, Address 0
        (1, 1, "coils", 0),   // Station 1, Type 01 Coils, Address 0
    ];

    log::info!("🧪 Step 2: Configuring 3 masters on {port1} with same station ID");
    for (i, &(station_id, register_type, register_mode, start_address)) in
        masters.iter().enumerate()
    {
        // Setup port once for the first master, subsequent masters share the same port
        if i == 0 {
            setup_tui_port(&mut tui_session, &mut tui_cap, &port1).await?;
        }

        // For second and subsequent masters, create a new station first
        if i > 0 {
            log::info!("➕ Creating new station entry for Master {}", i + 1);
            use ci_utils::auto_cursor::{execute_cursor_actions, CursorAction};
            use ci_utils::key_input::ArrowKey;
            let actions = vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Up,
                    count: 30,
                },
                CursorAction::Sleep { ms: 500 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 1000 },
            ];
            execute_cursor_actions(
                &mut tui_session,
                &mut tui_cap,
                &actions,
                &format!("create_station_for_master_{}", i + 1),
            )
            .await?;
        }

        configure_tui_master_common(
            &mut tui_session,
            &mut tui_cap,
            station_id,
            register_type,
            register_mode,
            start_address,
            REGISTER_LENGTH,
            i == 0, // is_first_station
        )
        .await?;
    }

    // Generate and update register data for all masters
    let master_data: Vec<Vec<u16>> = (0..3)
        .map(|_| generate_random_registers(REGISTER_LENGTH))
        .collect();

    log::info!("🧪 Updating all master registers");
    for (i, data) in master_data.iter().enumerate() {
        let (_, register_type, register_mode, _) = masters[i];
        log::info!(
            "  Master {} (Type {register_type}, {register_mode}) data: {data:?}",
            i + 1
        );
        update_tui_registers(&mut tui_session, &mut tui_cap, data, false).await?;
    }

    // Wait for IPC updates to propagate
    log::info!("🧪 Waiting for IPC propagation...");
    tokio::time::sleep(Duration::from_millis(3000)).await;

    // Test all 3 register types from vcom2
    let mut register_type_success = std::collections::HashMap::new();

    for (i, &(station_id, register_type, register_mode, start_address)) in
        masters.iter().enumerate()
    {
        log::info!("🧪 Testing Station {station_id} (Type {register_type}, {register_mode})");
        register_type_success.insert(
            register_type,
            test_station_with_retries(
                &port2,
                station_id,
                register_mode,
                start_address,
                &master_data[i],
                MAX_RETRIES,
                RETRY_INTERVAL_MS,
            )
            .await?,
        );
    }

    // Check if all register types passed
    let all_passed = register_type_success.values().all(|&v| v);

    if all_passed {
        log::info!("✅ All register types passed!");
        for (register_type, success) in register_type_success.iter() {
            log::info!(
                "  Register Type {register_type}: {}",
                if *success { "✅ PASS" } else { "❌ FAIL" }
            );
        }
    } else {
        log::error!("❌ Some register types failed:");
        for (register_type, success) in register_type_success.iter() {
            log::error!(
                "  Register Type {register_type}: {}",
                if *success { "✅ PASS" } else { "❌ FAIL" }
            );
        }
        return Err(anyhow!("Not all register types passed the test"));
    }

    // Clean up TUI process
    log::info!("🧪 Cleaning up TUI process");
    tui_session.send_ctrl_c()?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!("✅ TUI Multi-Masters Same Station test completed successfully!");
    Ok(())
}
