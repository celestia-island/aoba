use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::utils::{
    configure_tui_master_common, navigate_to_modbus_panel, test_station_with_retries,
};
use ci_utils::{
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ExpectKeyExt,
    ports::{port_exists, should_run_vcom_tests_with_ports, vcom_matchers_with_ports},
    snapshot::TerminalCapture,
    terminal::spawn_expect_process,
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
    const REGISTER_LENGTH: usize = 12;
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
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI Masters process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(60, 80); // Increased height to show all 4 stations

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

    log::info!("🧪 Step 2: Configuring and updating 4 masters on {port1}");

    // Navigate to port and enter Modbus panel (without enabling the port yet)
    navigate_to_modbus_panel(&mut tui_session, &mut tui_cap, &port1).await?;

    for (i, &(station_id, register_type, register_mode, start_address)) in
        masters.iter().enumerate()
    {
        log::info!(
            "🔧 Configuring Master {} (Station {}, Type {:02}, Addr 0x{:04X})",
            i + 1,
            station_id,
            register_type,
            start_address
        );

        // For second and subsequent masters, create a new station first
        if i > 0 {
            log::info!("➕ Creating new station entry for Master {}", i + 1);
            // Navigate to "Create Station" button and press Enter
            use ci_utils::auto_cursor::{execute_cursor_actions, CursorAction};
            use ci_utils::key_input::ArrowKey;
            let actions = vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Up,
                    count: 30, // Ensure we're at the top
                },
                CursorAction::Sleep { ms: 500 },
                CursorAction::PressEnter, // Press Enter on "Create Station"
                CursorAction::Sleep { ms: 1000 }, // Wait for station to be created
            ];
            execute_cursor_actions(
                &mut tui_session,
                &mut tui_cap,
                &actions,
                &format!("create_station_for_master_{}", i + 1),
            )
            .await?;
            log::info!("✅ New station created, cursor should now be on it");
        }

        // Configure the station (skip creation since it's already done)
        configure_tui_master_common(
            &mut tui_session,
            &mut tui_cap,
            station_id,
            register_type,
            register_mode,
            start_address,
            REGISTER_LENGTH,
            i == 0, // is_first_station: true only for the first master
        )
        .await?;

        // Immediately update data for this master
        log::info!("📝 Updating Master {} data: {:?}", i + 1, master_data[i]);
        update_tui_registers(&mut tui_session, &mut tui_cap, &master_data[i], false).await?;

        // Save this master's configuration with Ctrl+S to commit changes to IPC
        log::info!("💾 Saving Master {} configuration with Ctrl+S...", i + 1);
        use ci_utils::auto_cursor::{execute_cursor_actions, CursorAction};
        let actions = vec![
            CursorAction::PressCtrlS,
            CursorAction::Sleep { ms: 1000 }, // Wait for save operation
        ];
        execute_cursor_actions(
            &mut tui_session,
            &mut tui_cap,
            &actions,
            &format!("save_master_{}_config", i + 1),
        )
        .await?;

        // Wait for register updates to be saved and IPC to propagate before configuring next master
        log::info!(
            "⏱️ Waiting for Master {} updates to be fully saved and propagated...",
            i + 1
        );
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }

    // All Masters configured and saved individually, port should be enabled
    log::info!("✅ All Masters configured and saved, verifying port is enabled...");

    // Verify port is enabled by checking the status indicator in the top-right corner
    log::info!("🔍 Verifying port is enabled");
    let status = ci_utils::verify_port_enabled(
        &mut tui_session,
        &mut tui_cap,
        "verify_port_enabled_multi_masters",
    )
    .await?;
    log::info!("✅ Port enabled with status: {}, ready for testing", status);

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
