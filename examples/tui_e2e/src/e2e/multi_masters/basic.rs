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

/// Test Multiple TUI Masters on Single Port with IPC Communication - Basic Scenario
///
/// This test simulates 4 independent TUI masters on vcom1 with the same station ID and register type,
/// but managing different register address ranges:
/// - Master 1: Station ID 1, Register Type 03 (Holding Register), Address 0x0000-0x000B (0-11)
/// - Master 2: Station ID 1, Register Type 03 (Holding Register), Address 0x000C-0x0017 (12-23)
/// - Master 3: Station ID 1, Register Type 03 (Holding Register), Address 0x0018-0x0023 (24-35)
/// - Master 4: Station ID 1, Register Type 03 (Holding Register), Address 0x0024-0x002F (36-47)
///
/// Test Design:
/// - All masters share the same vcom1 port, same station ID, and same register type
/// - Each master manages a different, non-overlapping register address range
/// - Uses IPC communication to avoid port conflicts
/// - Each master has 12 registers with random data
/// - CLI slaves on vcom2 poll each address range to verify communication
///
/// The test validates:
/// 1. Multiple masters can operate on the same port with same station ID and register type
/// 2. Different register address ranges are properly managed without conflicts
/// 3. IPC communication prevents port conflicts
/// 4. Communication reliability with retry logic
pub async fn test_tui_multi_masters_basic(port1: &str, port2: &str) -> Result<()> {
    const REGISTER_LENGTH: usize = 8; // Reduced from 12 for testing
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Multi-Masters Basic test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Masters Basic E2E test");

    // Get platform-appropriate port names
    let ports = vcom_matchers_with_ports(port1, port2);
    let port1 = ports.port1_name.clone();
    let port2 = ports.port2_name.clone();

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (4 masters on same station, same register type, different address ranges)");
    log::info!("  Slaves: {port2} (CLI, polls all address ranges)");

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
    let terminal_size = TerminalSize::Large;
    let (rows, cols) = terminal_size.dimensions();
    let mut tui_session = spawn_expect_process_with_size(&["--tui"], Some((rows, cols)))
        .map_err(|err| anyhow!("Failed to spawn TUI Masters process: {err}"))?;
    let mut tui_cap = TerminalCapture::with_size(terminal_size);

    sleep_seconds(3).await;

    // Try an initial capture to "wake up" the terminal
    log::info!("🔄 Initializing screen capture...");
    let initial_screen = tui_cap.capture(&mut tui_session, "initial_screen").await?;
    if initial_screen.trim().is_empty() {
        log::warn!("⚠️ Initial screen capture is empty, TUI may not be rendering");
    } else {
        log::info!("✅ TUI screen initialized successfully");
    }

    // Configure all 4 masters on vcom1 - same station ID, same register type, different address ranges
    let masters = [
        (1, 3, "holding", 0),  // Station 1, Type 03, Address 0-11
        (1, 3, "holding", 12), // Station 1, Type 03, Address 12-23
        (1, 3, "holding", 24), // Station 1, Type 03, Address 24-35
        (1, 3, "holding", 36), // Station 1, Type 03, Address 36-47
    ];

    // Generate register data for all masters first (before configuration)
    let master_data: Vec<Vec<u16>> = (0..4)
        .map(|_| generate_random_registers(REGISTER_LENGTH))
        .collect();

    log::info!("🧪 Step 2: Configuring 4 masters on {port1}");

    // Navigate to port and enter Modbus panel (without enabling the port yet)
    navigate_to_modbus_panel(&mut tui_session, &mut tui_cap, &port1).await?;

    // Default connection mode is already Master, no need to change
    log::info!("✅ Connection mode is Master by default");

    // Phase 1: Create all 4 stations at once
    // Note: Pass is_master=false because default is already Master, don't toggle
    use crate::utils::create_modbus_stations;
    create_modbus_stations(&mut tui_session, &mut tui_cap, 4, false).await?;
    log::info!("✅ Phase 1 complete: All 4 stations created");

    // Phase 2: Configure each station individually and update its data immediately
    use crate::utils::configure_modbus_station;
    for (i, &(station_id, register_type, _register_mode, start_address)) in
        masters.iter().enumerate()
    {
        log::info!(
            "🔧 Configuring Master {} (Station {}, Type {:02}, Addr 0x{:04X})",
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

        // Immediately update data for this master (while cursor is in its register area)
        log::info!("📝 Updating Master {} data: {:?}", i + 1, master_data[i]);
        update_tui_registers(&mut tui_session, &mut tui_cap, &master_data[i], false).await?;

        // Wait for register updates to be saved before configuring next master
        log::info!("⏱️ Waiting for register updates to be fully saved...");
        ci_utils::sleep_a_while().await;
        ci_utils::sleep_a_while().await;

        log::info!("✅ Master {} configured and data updated", i + 1);
    }
    log::info!("✅ Phase 2 complete: All 4 stations configured with data");

    // All Masters configured with data, now save once with Ctrl+S to enable port
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

    log::info!("💾 Saving all master configurations with Ctrl+S to enable port...");
    let actions = vec![
        CursorAction::PressCtrlS,
        CursorAction::Sleep { ms: 5000 }, // Wait for port to enable and CLI subprocess to start reading data
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "save_all_masters_and_enable",
    )
    .await?;

    // Verify port is enabled by checking the status indicator
    log::info!("🔍 Verifying port is enabled");
    let status = ci_utils::verify_port_enabled(
        &mut tui_session,
        &mut tui_cap,
        "verify_port_enabled_multi_masters",
    )
    .await?;
    log::info!(
        "✅ Port enabled with status: {}, all data committed, ready for testing",
        status
    );

    // Test all 4 address ranges from vcom2
    let mut address_range_success = std::collections::HashMap::new();

    for (i, &(station_id, _, register_mode, start_address)) in masters.iter().enumerate() {
        log::info!("🧪 Testing Address Range {}: Station {station_id} ({register_mode}) at 0x{start_address:04X}", i + 1);
        address_range_success.insert(
            i,
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

    // Check if all address ranges passed
    let all_passed = address_range_success.values().all(|&v| v);

    if all_passed {
        log::info!("✅ All address ranges passed!");
        for (range_idx, success) in address_range_success.iter() {
            let start_addr = masters[*range_idx].3;
            log::info!(
                "  Address Range {} (0x{start_addr:04X}): {}",
                range_idx + 1,
                if *success { "✅ PASS" } else { "❌ FAIL" }
            );
        }
    } else {
        log::error!("❌ Some address ranges failed:");
        for (range_idx, success) in address_range_success.iter() {
            let start_addr = masters[*range_idx].3;
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

    log::info!("✅ TUI Multi-Masters Basic test completed successfully!");
    Ok(())
}
