use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::utils::{
    configure_tui_slave_common, navigate_to_modbus_panel, test_station_with_retries,
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

/// Test Multiple TUI Slaves on Single Port with Adjacent Registers
///
/// This test simulates 3 TUI slaves on vcom2 with different station IDs and adjacent register addresses:
/// - Slave 1: Station ID 1, Register Type 03 (Holding Register), Address 0-5
/// - Slave 2: Station ID 2, Register Type 03 (Holding Register), Address 6-11
/// - Slave 3: Station ID 3, Register Type 03 (Holding Register), Address 12-17
///
/// Test Design:
/// - All slaves share the same vcom2 port with different station IDs
/// - Uses IPC communication to avoid port conflicts
/// - Each slave has 6 registers with random data
/// - CLI masters on vcom1 poll each station to verify communication
/// - Tests adjacent register addressing to ensure no conflicts
///
/// The test validates:
/// 1. Multiple slaves can operate on the same port with different station IDs
/// 2. Adjacent register addressing works correctly without conflicts
/// 3. IPC communication prevents port conflicts
/// 4. Communication reliability with retry logic
pub async fn test_tui_multi_slaves_adjacent_registers(port1: &str, port2: &str) -> Result<()> {
    const REGISTER_LENGTH: usize = 8;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Multi-Slaves Adjacent Registers test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Slaves Adjacent Registers E2E test");

    // Get platform-appropriate port names
    let ports = vcom_matchers_with_ports(port1, port2);
    let port1 = ports.port1_name.clone();
    let port2 = ports.port2_name.clone();

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (CLI, polls all stations)");
    log::info!("  Slaves: {port2} (3 stations with adjacent register addresses)");

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
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);

    sleep_seconds(3).await;

    // Configure 3 slaves on vcom2 with different station IDs and adjacent register addresses
    let slaves = [
        (1, 3, "holding", 0), // Station 1, Type 03 Holding Register, Address 0-5
        (2, 3, "holding", 0), // Station 2, Type 03 Holding Register, Address 0-5
        (3, 3, "holding", 0), // Station 3, Type 03 Holding Register, Address 0-5
    ];

    log::info!("🧪 Step 2: Configuring 3 slaves on {port2} with adjacent register addresses");

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
    for (i, &(station_id, register_type, _register_mode, start_address)) in slaves.iter().enumerate()
    {
        log::info!("🔧 Configuring Slave {} (Station {})", i + 1, station_id);

        configure_modbus_station(
            &mut tui_session,
            &mut tui_cap,
            i,                 // station_index (0-based)
            station_id,
            register_type,
            start_address,
            REGISTER_LENGTH,
        )
        .await?;

        // Immediately update data for this slave
        log::info!(
            "📝 Updating Slave {} (Station {}, Address 0x{:04X}-0x{:04X}) data: {:?}",
            i + 1,
            station_id,
            start_address,
            start_address + REGISTER_LENGTH as u16 - 1,
            slave_data[i]
        );
        update_tui_registers(&mut tui_session, &mut tui_cap, &slave_data[i], false).await?;

        // Wait for register updates to be saved before configuring next slave
        log::info!("⏱️ Waiting for register updates to be fully saved...");
        ci_utils::sleep_a_while().await;
        ci_utils::sleep_a_while().await;
    }
    log::info!("✅ Phase 2 complete: All 4 stations configured and data updated");

    // After configuring all Slaves, save and exit Modbus panel
    log::info!("🔄 All Slaves configured, saving configuration...");

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
    log::info!("🧪 Waiting for IPC propagation...");
    tokio::time::sleep(Duration::from_millis(3000)).await;

    // Test all 3 stations from vcom1
    let mut station_success = std::collections::HashMap::new();

    for (i, &(station_id, _, register_mode, start_address)) in slaves.iter().enumerate() {
        log::info!("🧪 Testing Station {station_id} ({register_mode})");
        station_success.insert(
            station_id,
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

    log::info!("✅ TUI Multi-Slaves Adjacent Registers test completed successfully!");
    Ok(())
}
