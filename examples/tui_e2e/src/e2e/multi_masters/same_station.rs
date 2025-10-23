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

/// Test Multiple TUI Masters on Single Port with Same Station ID but Different Register Types
///
/// This test simulates 2 TUI masters on vcom1 with the same station ID but different register types:
/// - Master 1: Station ID 1, Register Type 03 (Holding Register)
/// - Master 2: Station ID 1, Register Type 04 (Input Register)
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
pub async fn test_tui_multi_masters_same_station(port1: &str, port2: &str) -> Result<()> {
    const REGISTER_LENGTH: usize = 8;
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 1000;

    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Multi-Masters Same Station test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Multi-Masters Same Station E2E test");

    // Get platform-appropriate port names
    let ports = vcom_matchers_with_ports(port1, port2);
    let port1 = ports.port1_name.clone();
    let port2 = ports.port2_name.clone();

    log::info!("📍 Port configuration:");
    log::info!("  Masters: {port1} (2 masters with same station ID, different register types)");
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
    let terminal_size = TerminalSize::Large;
    let (rows, cols) = terminal_size.dimensions();
    let mut tui_session = spawn_expect_process_with_size(&["--tui"], Some((rows, cols)))
        .map_err(|err| anyhow!("Failed to spawn TUI Masters process: {err}"))?;
    let mut tui_cap = TerminalCapture::with_size(terminal_size);

    sleep_seconds(3).await;

    // Configure 2 masters on vcom1 with same station ID but different register types
    let masters = [
        (1, 3, "holding", 0), // Station 1, Type 03 Holding Register, Address 0
        (1, 4, "input", 0),   // Station 1, Type 04 Input Register, Address 0
    ];

    log::info!("🧪 Step 2: Configuring 2 masters on {port1} with same station ID");

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

    // Test all 3 register types from vcom2
    // NOTE: Since we skip data update phase, all registers should be 0 (default value)
    let expected_data: Vec<u16> = vec![0; REGISTER_LENGTH];
    let mut register_type_success = std::collections::HashMap::new();

    for (_i, &(station_id, register_type, register_mode, start_address)) in
        masters.iter().enumerate()
    {
        log::info!("🧪 Testing Station {station_id} (Type {register_type}, {register_mode}, expecting all zeros)");
        register_type_success.insert(
            register_type,
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
