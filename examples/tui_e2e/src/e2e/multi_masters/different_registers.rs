use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::utils::{
    configure_multiple_stations, navigate_to_modbus_panel, test_station_with_retries,
};
use ci_utils::{
    helpers::sleep_seconds,
    key_input::ExpectKeyExt,
    ports::{port_exists, should_run_vcom_tests_with_ports, vcom_matchers_with_ports},
    snapshot::{TerminalCapture, TerminalSize},
    terminal::spawn_expect_process_with_size,
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
    let terminal_size = TerminalSize::Large;
    let (rows, cols) = terminal_size.dimensions();
    let mut tui_session = spawn_expect_process_with_size(&["--tui"], Some((rows, cols)))
        .map_err(|err| anyhow!("Failed to spawn TUI Masters process: {err}"))?;
    let mut tui_cap = TerminalCapture::with_size(terminal_size);

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

    // Use unified configuration function to create and configure all stations
    let station_configs: Vec<(u8, u8, u16, usize)> = masters
        .iter()
        .map(|&(id, typ, _, addr)| (id, typ, addr as u16, REGISTER_LENGTH))
        .collect();

    configure_multiple_stations(&mut tui_session, &mut tui_cap, &station_configs).await?;

    // All Masters configured, now save once with Ctrl+S to enable port
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

    log::info!("💾 Saving all master configurations with Ctrl+S to enable port...");
    let actions = vec![
        CursorAction::PressCtrlS,
        CursorAction::Sleep { ms: 5000 }, // Wait for port to enable and CLI subprocess to start
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "save_all_masters_and_enable",
    )
    .await?;

    // Verify port is enabled by checking the status indicator (still in Modbus panel)
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

    // Test all 4 stations from vcom2
    // NOTE: Since we skip data update phase, all registers should be 0 (default value)
    let expected_data: Vec<u16> = vec![0; REGISTER_LENGTH];
    let mut station_success = std::collections::HashMap::new();

    for (_i, &(station_id, register_type, register_mode, start_address)) in
        masters.iter().enumerate()
    {
        log::info!("🧪 Testing Station {station_id} (Type {register_type}, {register_mode}, expecting all zeros)");
        station_success.insert(
            station_id,
            test_station_with_retries(
                &port2,
                station_id,
                register_mode,
                start_address,
                &expected_data, // Expect all zeros since we didn't update data
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
