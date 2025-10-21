use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::utils::{navigate_to_modbus_panel, test_station_with_retries};
use ci_utils::{
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ExpectKeyExt,
    ports::{port_exists, should_run_vcom_tests_with_ports, vcom_matchers_with_ports},
    snapshot::{TerminalCapture, TerminalSize},
    terminal::spawn_expect_process,
    tui::update_tui_registers,
};

/// Test Multiple TUI Slaves on Single Port with IPC Communication - Basic Scenario
///
/// This test simulates 4 independent TUI slaves on vcom2 with the same station ID and register type,
/// but managing different register address ranges:
/// - Slave 1: Station ID 1, Register Type 03 (Holding Register), Address 0x0000-0x0007 (0-7)
/// - Slave 2: Station ID 1, Register Type 03 (Holding Register), Address 0x0008-0x000F (8-15)
/// - Slave 3: Station ID 1, Register Type 03 (Holding Register), Address 0x0010-0x0017 (16-23)
/// - Slave 4: Station ID 1, Register Type 03 (Holding Register), Address 0x0018-0x001F (24-31)
///
/// Test Design:
/// - All slaves share the same vcom2 port, same station ID, and same register type
/// - Each slave manages a different, non-overlapping register address range
/// - Uses IPC communication to avoid port conflicts
/// - Each slave has 8 registers with random data
/// - CLI masters on vcom1 poll each address range to verify communication
///
/// The test validates:
/// 1. Multiple slaves can operate on the same port with same station ID and register type
/// 2. Different register address ranges are properly managed without conflicts
/// 3. IPC communication prevents port conflicts
/// 4. Communication reliability with retry logic
pub async fn test_tui_multi_slaves_basic(port1: &str, port2: &str) -> Result<()> {
    const REGISTER_LENGTH: usize = 8;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Multi-Slaves Basic test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Slaves Basic E2E test");

    // Get platform-appropriate port names
    let ports = vcom_matchers_with_ports(port1, port2);
    let port1 = ports.port1_name.clone();
    let port2 = ports.port2_name.clone();

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (CLI, polls all address ranges)");
    log::info!("  Slaves: {port2} (4 slaves on same station, same register type, different address ranges)");

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
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Large); // Increased height to show all 4 stations

    sleep_seconds(3).await;

    // Configure all 4 slaves on vcom2 - same station ID, same register type, different address ranges
    let slaves = [
        (1, 3, "holding", 0),  // Station 1, Type 03, Address 0-7
        (1, 3, "holding", 8),  // Station 1, Type 03, Address 8-15
        (1, 3, "holding", 16), // Station 1, Type 03, Address 16-23
        (1, 3, "holding", 24), // Station 1, Type 03, Address 24-31
    ];

    // Generate register data for all slaves first (before configuration)
    let slave_data: Vec<Vec<u16>> = (0..4)
        .map(|_| generate_random_registers(REGISTER_LENGTH))
        .collect();

    log::info!("🧪 Step 2: Configuring 4 slaves on {port2}");

    // Navigate to port and enter Modbus panel (without enabling the port yet)
    navigate_to_modbus_panel(&mut tui_session, &mut tui_cap, &port2).await?;

    // Phase 1: Create all 4 stations at once
    use crate::utils::create_modbus_stations;
    create_modbus_stations(&mut tui_session, &mut tui_cap, 4, false).await?; // false = slave mode
    log::info!("✅ Phase 1 complete: All 4 stations created");

    // Phase 2: Configure each station individually
    use crate::utils::configure_modbus_station;
    for (i, &(station_id, register_type, _register_mode, start_address)) in
        slaves.iter().enumerate()
    {
        log::info!(
            "🔧 Configuring Slave {} (Station {}, Type {:02}, Addr 0x{:04X})",
            i + 1,
            station_id,
            register_type,
            start_address
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

        log::info!("✅ Slave {} configured", i + 1);
    }
    log::info!("✅ Phase 2 complete: All 4 stations configured");

    // All Slaves configured, now save once with Ctrl+S to enable port and commit all changes
    // First, navigate to the top of the panel to ensure we're not in edit mode
    log::info!("📍 Navigating to top of panel before saving...");
    use ci_utils::auto_cursor::{execute_cursor_actions, CursorAction};
    use ci_utils::key_input::ArrowKey;
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
        CursorAction::Sleep { ms: 5000 }, // Increased wait time for port to enable and stabilize
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "save_all_slaves_and_enable",
    )
    .await?;

    // After configuring all Slaves, verify port is enabled
    log::info!("✅ All Slaves configured and saved, verifying port is enabled...");

    // Verify port is enabled by checking the status indicator
    log::info!("🔍 Verifying port is enabled");
    let status = ci_utils::verify_port_enabled(
        &mut tui_session,
        &mut tui_cap,
        "verify_port_enabled_multi_slaves",
    )
    .await?;
    log::info!("✅ Port enabled with status: {}, ready for testing", status);

    // Now update register data for all slaves after port is enabled
    for (i, &(_station_id, _register_type, _register_mode, start_address)) in
        slaves.iter().enumerate()
    {
        log::info!(
            "📝 Updating Slave {} data at address 0x{:04X}: {:?}",
            i + 1,
            start_address,
            slave_data[i]
        );

        // Navigate to the specific station before updating its registers
        if i > 0 {
            execute_cursor_actions(
                &mut tui_session,
                &mut tui_cap,
                &[
                    CursorAction::PressCtrlPageUp,
                    CursorAction::Sleep { ms: 300 },
                    CursorAction::PressPageDown,
                    CursorAction::Sleep { ms: 300 },
                    CursorAction::PressArrow {
                        direction: ArrowKey::Down,
                        count: i * 5, // Each station takes ~5 cursor positions
                    },
                    CursorAction::Sleep { ms: 300 },
                ],
                &format!("nav_to_station_{}_for_update", i + 1),
            )
            .await?;
        } else {
            let nav_to_station_actions = vec![
                CursorAction::PressCtrlPageUp,
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressPageDown,
                CursorAction::Sleep { ms: 300 },
            ];
            execute_cursor_actions(
                &mut tui_session,
                &mut tui_cap,
                &nav_to_station_actions,
                "nav_to_first_station_for_update",
            )
            .await?;
        }

        update_tui_registers(&mut tui_session, &mut tui_cap, &slave_data[i], false).await?;
        log::info!("✅ Slave {} data updated", i + 1);
    }

    // Test all 4 address ranges from vcom1
    let mut address_range_success = std::collections::HashMap::new();

    for (i, &(station_id, _, register_mode, start_address)) in slaves.iter().enumerate() {
        log::info!("🧪 Testing Address Range {}: Station {station_id} ({register_mode}) at 0x{start_address:04X}", i+1);
        address_range_success.insert(
            i,
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

    // Check if all address ranges passed
    let all_passed = address_range_success.values().all(|&v| v);

    if all_passed {
        log::info!("✅ All address ranges passed!");
        for (range_idx, success) in address_range_success.iter() {
            let start_addr = slaves[*range_idx].3;
            log::info!(
                "  Address Range {} (0x{start_addr:04X}): {}",
                range_idx + 1,
                if *success { "✅ PASS" } else { "❌ FAIL" }
            );
        }
    } else {
        log::error!("❌ Some address ranges failed:");
        for (range_idx, success) in address_range_success.iter() {
            let start_addr = slaves[*range_idx].3;
            log::error!(
                "  Address Range {} (0x{start_addr:04X}): {}",
                range_idx + 1,
                if *success { "✅ PASS" } else { "❌ FAIL" }
            );
        }
        return Err(anyhow!("Not all address ranges passed the test"));
    }

    // Clean up TUI process
    log::info!("🧪 Cleaning up TUI process");
    tui_session.send_ctrl_c()?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!("✅ TUI Multi-Slaves Basic test completed successfully!");
    Ok(())
}
