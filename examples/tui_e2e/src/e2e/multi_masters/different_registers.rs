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
    snapshot::{TerminalCapture, TerminalSize},
    terminal::spawn_expect_process,
    tui::update_tui_registers,
};
use expectrl::Expect;

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
pub async fn test_tui_multi_masters_different_registers(port1: &str, port2: &str) -> Result<()> {
    const REGISTER_LENGTH: usize = 8;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Multi-Masters Different Registers test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Masters Different Registers E2E test");

    // Get platform-appropriate port names
    let ports = vcom_matchers_with_ports(port1, port2);
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
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);

    sleep_seconds(3).await;

    // Configure 4 masters on vcom1 with different station IDs and register types
    let masters = [
        (1, 3, "holding", 0),  // Station 1, Type 03 Holding Register, Address 0
        (2, 4, "input", 0),    // Station 2, Type 04 Input Register, Address 0
        (3, 1, "coils", 0),    // Station 3, Type 01 Coils, Address 0
        (4, 2, "discrete", 0), // Station 4, Type 02 Discrete Inputs, Address 0
    ];

    log::info!("🧪 Step 2: Configuring 4 masters on {port1} with different register types");

    // Navigate to port and enter Modbus panel (without enabling the port yet)
    navigate_to_modbus_panel(&mut tui_session, &mut tui_cap, &port1).await?;

    // Generate register data for all masters first (before configuration)
    let master_data: Vec<Vec<u16>> = (0..4)
        .map(|_| generate_random_registers(REGISTER_LENGTH))
        .collect();

    for (i, &(station_id, register_type, register_mode, start_address)) in
        masters.iter().enumerate()
    {
        log::info!(
            "🔧 Configuring Master {} (Station {}, Type {:02})",
            i + 1,
            station_id,
            register_type
        );

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

        // Immediately update data for this master
        log::info!("📝 Updating Master {} data: {:?}", i + 1, master_data[i]);
        update_tui_registers(&mut tui_session, &mut tui_cap, &master_data[i], false).await?;

        // Wait for register updates to be saved before configuring next master
        log::info!("⏱️ Waiting for register updates to be fully saved...");
        ci_utils::sleep_a_while().await;
        ci_utils::sleep_a_while().await;
    }

    // After configuring all Masters, save and exit Modbus panel
    log::info!("🔄 All Masters configured, saving configuration...");

    // Send Esc to trigger handle_leave_page (auto-enable + switch to ConfigPanel)
    log::info!("⌨️ Sending Esc to trigger auto-enable and return to ConfigPanel...");
    tui_session.send("\x1b")?;
    ci_utils::sleep_a_while().await;
    ci_utils::sleep_a_while().await;

    // Verify we're at ConfigPanel (port details page) AND port is enabled with retry logic
    log::info!("⏳ Waiting for screen to update to ConfigPanel and port to be enabled...");
    let mut screen;
    let max_attempts = 3;
    let mut at_config_panel = false;
    let mut port_enabled = false;

    for attempt in 1..=max_attempts {
        ci_utils::sleep_a_while().await;
        screen = tui_cap
            .capture(
                &mut tui_session,
                &format!("after_save_modbus_attempt_{}", attempt),
            )
            .await?;

        // Check if we're at ConfigPanel
        if screen.contains("Enable Port") {
            at_config_panel = true;

            // Check if port is showing as Enabled
            for line in screen.lines() {
                if line.contains("Enable Port") && line.contains("Enabled") {
                    port_enabled = true;
                    break;
                }
            }

            if port_enabled {
                log::info!(
                    "✅ Port enabled and shown in UI on attempt {}/{}",
                    attempt,
                    max_attempts
                );
                break;
            } else {
                log::info!(
                    "⏳ Attempt {}/{}: At ConfigPanel but port not showing as Enabled yet, waiting for CLI subprocess...",
                    attempt,
                    max_attempts
                );
            }
        } else {
            log::warn!(
                "⏳ Attempt {}/{}: Not at ConfigPanel yet, waiting...",
                attempt,
                max_attempts
            );
        }
    }

    if !at_config_panel {
        return Err(anyhow!(
            "Failed to return to port details page after saving Modbus configuration (tried {} times)",
            max_attempts
        ));
    }

    if !port_enabled {
        return Err(anyhow!(
            "Port not showing as Enabled after {} attempts",
            max_attempts
        ));
    }

    log::info!("💾 Saved Modbus configuration and auto-enabled port");
    log::info!("✅ Successfully returned to port details page with port enabled");

    // Test all 4 stations from vcom2
    let mut station_success = std::collections::HashMap::new();

    for (i, &(station_id, register_type, register_mode, start_address)) in
        masters.iter().enumerate()
    {
        log::info!("🧪 Testing Station {station_id} (Type {register_type}, {register_mode})");
        station_success.insert(
            station_id,
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
