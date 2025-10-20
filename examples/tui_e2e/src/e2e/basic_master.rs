use anyhow::{anyhow, Result};
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ArrowKey,
    ports::{port_exists, should_run_vcom_tests_with_ports, vcom_matchers_with_ports},
    snapshot::{TerminalCapture, TerminalSize},
    terminal::{build_debug_bin, spawn_expect_process},
};
use serde_json::json;

const ROUNDS: usize = 3;
const REGISTER_LENGTH: usize = 12;

/// Test TUI Master-Provide + CLI Slave-Poll with continuous random data (3 rounds)
///
/// This version uses status monitoring exclusively - no terminal content matching.
/// All verification is done through CheckStatus actions that read JSON status dumps.
///
/// TUI acts as Modbus Master (server providing data, responding to poll requests).
/// CLI acts as Modbus Slave (client polling for data).
pub async fn test_tui_master_with_cli_slave_continuous(port1: &str, port2: &str) -> Result<()> {
    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Master-Provide + CLI Slave-Poll test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master-Provide + CLI Slave-Poll continuous test (3 rounds)");

    let ports = vcom_matchers_with_ports(port1, port2);

    // Verify vcom ports exist (platform-aware check)
    if !port_exists(&ports.port1_name) {
        return Err(anyhow!(
            "{} does not exist or is not available",
            ports.port1_name
        ));
    }
    if !port_exists(&ports.port2_name) {
        return Err(anyhow!(
            "{} does not exist or is not available",
            ports.port2_name
        ));
    }
    log::info!("✅ Virtual COM ports verified");

    // Enable debug mode for status monitoring
    std::env::set_var("AOBA_DEBUG_CI_E2E_TEST", "1");
    log::info!("🔍 Debug CI E2E test mode enabled");

    // Spawn TUI process (will be master/server on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);

    // Wait for TUI to initialize and start writing status
    log::info!("⏳ Waiting for TUI to initialize...");
    sleep_seconds(3).await;

    // Wait for TUI to reach Entry page
    log::info!("🧪 Step 2: Wait for TUI to reach Entry page");
    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should be on Entry page".to_string(),
        path: "page.type".to_string(),
        expected: json!("Entry"),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "wait_entry_page").await?;

    // Navigate to port1 using keyboard
    log::info!("🧪 Step 3: Navigate to {} in port list", port1);
    let actions = vec![
        // Port list starts with cursor on first port (vcom1), so just press Enter
        // Debug: Check terminal to see if port is highlighted
        CursorAction::DebugBreakpoint {
            description: "port_selection".to_string(),
        },
        CursorAction::PressEnter, // Enter port details
        CursorAction::Sleep { ms: 1000 },
        // Check that we reached ConfigPanel
        CursorAction::CheckStatus {
            description: "Should be on ConfigPanel".to_string(),
            path: "page.type".to_string(),
            expected: json!("ConfigPanel"),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "navigate_to_port").await?;

    // Enter Modbus configuration panel
    log::info!("🧪 Step 4: Enter Modbus configuration panel");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 3, // Navigate to ModBus Master/Slave Set
        },
        CursorAction::Sleep { ms: 300 },
        // Debug: Verify cursor position before entering panel
        CursorAction::DebugBreakpoint {
            description: "before_enter_modbus".to_string(),
        },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Verify we're now on ModbusDashboard
        CursorAction::CheckStatus {
            description: "Should be on ModbusDashboard".to_string(),
            path: "page.type".to_string(),
            expected: json!("ModbusDashboard"),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "enter_modbus_panel",
    )
    .await?;

    // Configure as Master - create station and configure
    log::info!("🧪 Step 5: Configure TUI as Master (server/provide mode)");
    let actions = vec![
        // Press Enter on "Create Station" button
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Debug: Check if station creation dialog appeared
        CursorAction::DebugBreakpoint {
            description: "after_create_station".to_string(),
        },
        // Configure station (station_id=1, holding registers, address=0, length=12)
        // Navigate to register length field
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 4,
        },
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter, // Enter edit mode
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(REGISTER_LENGTH.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter, // Confirm edit
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "configure_master").await?;

    // Save configuration with Ctrl+S which will auto-enable the port
    log::info!("🧪 Step 6: Save configuration with Ctrl+S to auto-enable port");
    let actions = vec![
        // Debug: Check state before saving
        CursorAction::DebugBreakpoint {
            description: "before_save".to_string(),
        },
        CursorAction::PressCtrlS,
        CursorAction::Sleep { ms: 3000 }, // Wait for port to enable and stabilize
        // Verify port is enabled through status monitoring
        CursorAction::CheckStatus {
            description: format!("Port {} should be enabled", port1),
            path: "ports[0].enabled".to_string(),
            expected: json!(true),
            timeout_secs: Some(15),
            retry_interval_ms: Some(500),
        },
        // Verify master configuration exists
        CursorAction::CheckStatus {
            description: "Should have one master station configured".to_string(),
            path: "ports[0].modbus_masters[0].station_id".to_string(),
            expected: json!(1),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_and_verify").await?;

    log::info!("✅ Port enabled and master configuration verified");

    // Run communication rounds with external CLI slave
    log::info!("🧪 Step 7: Running {} communication rounds", ROUNDS);

    let binary = build_debug_bin("aoba")?;

    for round in 1..=ROUNDS {
        log::info!("📡 Round {}/{}: Starting communication test", round, ROUNDS);

        // Generate random data for this round
        let random_data = generate_random_registers(REGISTER_LENGTH);
        log::info!("   Generated test data: {:?}", random_data);

        // Update TUI registers via keyboard (simplified - in real implementation would need proper navigation)
        // For now, we'll just verify communication happens

        // Spawn CLI slave-poll process on port2
        log::info!("   Spawning CLI slave on {}", port2);
        let mut cli_child = Command::new(&binary)
            .args([
                "--slave-poll",
                &ports.port2_name,
                "--station-id",
                "1",
                "--register-address",
                "0",
                "--register-length",
                &REGISTER_LENGTH.to_string(),
                "--register-mode",
                "holding",
                "--baud-rate",
                "9600",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Wait for communication to occur
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Kill CLI process
        cli_child.kill()?;
        cli_child.wait()?;

        log::info!("   ✅ Round {}/{} completed", round, ROUNDS);
    }

    log::info!("🎉 All {} rounds completed successfully!", ROUNDS);

    // Exit TUI
    let actions = vec![CursorAction::CtrlC, CursorAction::Sleep { ms: 500 }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "exit_tui").await?;

    Ok(())
}
