use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::utils::{navigate_to_modbus_panel, test_station_with_retries};
use ci_utils::{
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ExpectKeyExt,
    ports::{port_exists, should_run_vcom_tests_with_ports, vcom_matchers_with_ports},
    snapshot::{TerminalCapture, TerminalSize},
    terminal::spawn_expect_process_with_size,
    tui::update_tui_registers,
};
use expectrl::Expect;

/// Test Multiple TUI Slaves on Single Port with Same Station ID but Different Register Types
///
/// This test simulates 3 TUI slaves on vcom2 with the same station ID but different register types:
/// - Slave 1: Station ID 1, Register Type 03 (Holding Register)
/// - Slave 2: Station ID 1, Register Type 04 (Input Register)
/// - Slave 3: Station ID 1, Register Type 01 (Coils)
///
/// Test Design:
/// - All slaves share the same vcom2 port and same station ID but different register types
/// - Uses IPC communication to avoid port conflicts
/// - Each slave has 8 registers with random data
/// - CLI masters on vcom1 poll each register type to verify communication
///
/// The test validates:
/// 1. Multiple slaves can operate on the same port with same station ID but different register types
/// 2. IPC communication prevents port conflicts
/// 3. Different register types work correctly within the same station
/// 4. Communication reliability with retry logic
pub async fn test_tui_multi_slaves_same_station(port1: &str, port2: &str) -> Result<()> {
    const REGISTER_LENGTH: usize = 8;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Multi-Slaves Same Station test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Slaves Same Station E2E test");

    // Get platform-appropriate port names
    let ports = vcom_matchers_with_ports(port1, port2);
    let port1 = ports.port1_name.clone();
    let port2 = ports.port2_name.clone();

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (CLI, polls all register types)");
    log::info!("  Slaves: {port2} (3 slaves with same station ID, different register types)");

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
    let terminal_size = TerminalSize::Large;
    let (rows, cols) = terminal_size.dimensions();
    let mut tui_session = spawn_expect_process_with_size(&["--tui"], Some((rows, cols)))
        .map_err(|err| anyhow!("Failed to spawn TUI Slaves process: {err}"))?;
    let mut tui_cap = TerminalCapture::with_size(terminal_size);

    sleep_seconds(3).await;

    // Configure 3 slaves on vcom2 with same station ID but different register types
    let slaves = [
        (1, 3, "holding", 0), // Station 1, Type 03 Holding Register, Address 0
        (1, 4, "input", 0),   // Station 1, Type 04 Input Register, Address 0
        (1, 1, "coils", 0),   // Station 1, Type 01 Coils, Address 0
    ];

    log::info!("🧪 Step 2: Configuring 3 slaves on {port2} with same station ID");

    // Navigate to port and enter Modbus panel (without enabling the port yet)
    navigate_to_modbus_panel(&mut tui_session, &mut tui_cap, &port2).await?;

    // Generate register data for all slaves first (before configuration)
    let slave_data: Vec<Vec<u16>> = (0..3)
        .map(|_| generate_random_registers(REGISTER_LENGTH))
        .collect();

    // Phase 1: Create all 4 stations at once
    use crate::utils::create_modbus_stations;
    create_modbus_stations(&mut tui_session, &mut tui_cap, 4, false).await?; // false = slave mode
    log::info!("✅ Phase 1 complete: All 4 stations created");

    // Phase 2: Configure each station individually and update its data
    use crate::utils::configure_modbus_station;
    for (i, &(station_id, register_type, _register_mode, start_address)) in
        slaves.iter().enumerate()
    {
        log::info!(
            "🔧 Configuring Slave {} (Station {}, Type {:02})",
            i + 1,
            station_id,
            register_type
        );

        configure_modbus_station(
            &mut tui_session,
            &mut tui_cap,
            i, // station_index (0-based)
            station_id,
            register_type,
            start_address,
            REGISTER_LENGTH,
        )
        .await?;

        // Immediately update data for this slave
        log::info!("📝 Updating Slave {} data: {:?}", i + 1, slave_data[i]);
        update_tui_registers(&mut tui_session, &mut tui_cap, &slave_data[i], false).await?;

        // Wait for register updates to be saved before configuring next slave
        log::info!("⏱️ Waiting for register updates to be fully saved...");
        ci_utils::sleep_a_while().await;
        ci_utils::sleep_a_while().await;
    }
    log::info!("✅ Phase 2 complete: All 4 stations configured and data updated");

    // All Slaves configured with data, now save once with Ctrl+S to enable port
    log::info!("📍 Navigating to top of panel before saving...");
    use ci_utils::auto_cursor::{execute_cursor_actions, CursorAction};
    let nav_actions = vec![
        CursorAction::PressCtrlPageUp, // Jump to top (AddLine / Create Station)
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &nav_actions,
        "nav_to_top_before_save",
    )
    .await?;

    log::info!("💾 Saving all slave configurations with Ctrl+S to enable port...");
    let actions = vec![
        CursorAction::PressCtrlS,
        CursorAction::Sleep { ms: 5000 }, // Wait for port to enable and CLI subprocess to start reading data
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "save_all_slaves_and_enable",
    )
    .await?;

    // Verify port is enabled by checking the status indicator (still in Modbus panel)
    log::info!("🔍 Verifying port is enabled");
    let status = ci_utils::verify_port_enabled(
        &mut tui_session,
        &mut tui_cap,
        "verify_port_enabled_multi_slaves",
    )
    .await?;
    log::info!(
        "✅ Port enabled with status: {}, all data committed, ready for testing",
        status
    );

    // Test all 3 register types from vcom1
    let mut register_type_success = std::collections::HashMap::new();

    for (i, &(station_id, register_type, register_mode, start_address)) in slaves.iter().enumerate()
    {
        log::info!("🧪 Testing Station {station_id} (Type {register_type}, {register_mode})");
        register_type_success.insert(
            register_type,
            test_station_with_retries(
                &port1,
                station_id,
                register_mode,
                start_address,
                &slave_data[i],
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

    log::info!("✅ TUI Multi-Slaves Same Station test completed successfully!");
    Ok(())
}
