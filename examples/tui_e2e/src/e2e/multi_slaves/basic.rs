use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::utils::{configure_tui_slave_common, setup_tui_port, test_station_with_retries};
use ci_utils::{
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ExpectKeyExt,
    ports::{port_exists, should_run_vcom_tests},
    snapshot::TerminalCapture,
    terminal::spawn_expect_process,
    tui::{enter_modbus_panel, update_tui_registers},
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
pub async fn test_tui_multi_slaves_basic() -> Result<()> {
    const REGISTER_LENGTH: usize = 8;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Multi-Slaves Basic test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Slaves Basic E2E test");

    // Get platform-appropriate port names from ci_utils (handles env overrides and defaults)
    let ports = ci_utils::ports::vcom_matchers();
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
    let mut tui_cap = TerminalCapture::new(24, 80);

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

    log::info!("🧪 Step 2: Configuring and updating 4 slaves on {port2}");
    for (i, &(station_id, register_type, register_mode, start_address)) in slaves.iter().enumerate()
    {
        log::info!(
            "🔧 Configuring Slave {} (Station {}, Type {:02}, Addr 0x{:04X})",
            i + 1,
            station_id,
            register_type,
            start_address
        );

        // Setup port once for the first slave, subsequent slaves share the same port
        if i == 0 {
            setup_tui_port(&mut tui_session, &mut tui_cap, &port2).await?;
        } else {
            // For subsequent slaves, navigate back to Modbus panel to add another station
            log::info!("🔄 Navigating back to Modbus panel for Slave {}", i + 1);
            enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;
        }

        configure_tui_slave_common(
            &mut tui_session,
            &mut tui_cap,
            station_id,
            register_type,
            register_mode,
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
        ci_utils::sleep_a_while().await; // Extra delay to ensure last register is saved

        // Debug breakpoint: capture screen after configuring and updating each slave
        use ci_utils::auto_cursor::{execute_cursor_actions, CursorAction};
        if std::env::var("DEBUG_MODE").is_ok() {
            log::info!(
                "🔴 DEBUG: Capturing screen after Slave {} configuration",
                i + 1
            );
            let actions = vec![CursorAction::DebugBreakpoint {
                description: format!("after_slave_{}_config", i + 1),
            }];
            execute_cursor_actions(
                &mut tui_session,
                &mut tui_cap,
                &actions,
                &format!("debug_slave_{}", i + 1),
            )
            .await?;
        }
    }

    // Wait for IPC updates to propagate
    log::info!("🧪 Waiting for IPC propagation...");
    tokio::time::sleep(Duration::from_millis(3000)).await;

    // Debug breakpoint: capture screen before starting polls
    use ci_utils::auto_cursor::{execute_cursor_actions, CursorAction};
    if std::env::var("DEBUG_MODE").is_ok() {
        log::info!("🔴 DEBUG: Capturing screen before starting polls");
        let actions = vec![CursorAction::DebugBreakpoint {
            description: "before_polling_slaves".to_string(),
        }];
        execute_cursor_actions(
            &mut tui_session,
            &mut tui_cap,
            &actions,
            "debug_before_poll_slaves",
        )
        .await?;
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
