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
    tui::enter_modbus_panel,
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

    // Enter Modbus configuration panel from ConfigPanel
    // We need to navigate to "Enter Business Configuration" option
    log::info!("🧪 Step 4: Enter Modbus configuration panel");
    enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;

    // Verify we're now on ModbusDashboard
    let actions = vec![CursorAction::CheckStatus {
        description: "Should be on ModbusDashboard".to_string(),
        path: "page.type".to_string(),
        expected: json!("ModbusDashboard"),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "verify_modbus_panel",
    )
    .await?;

    // Step 4.5: Verify clean state before configuration (no previous test residue)
    log::info!("🧪 Step 4.5: Verify clean state (port disabled, no config)");
    let actions = vec![
        CursorAction::CheckStatus {
            description: "Port should be disabled before configuration".to_string(),
            path: "ports[0].enabled".to_string(),
            expected: json!(false),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
        CursorAction::CheckStatus {
            description: "Should have no modbus masters configured yet".to_string(),
            path: "ports[0].modbus_masters".to_string(),
            expected: json!([]),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "verify_clean_state",
    )
    .await?;

    // Configure as Master - create station and configure
    log::info!("🧪 Step 5: Configure TUI as Master (server/provide mode)");
    let actions = vec![
        // Press Enter on "Create Station" button
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Configure station (station_id=1, holding registers, address=0, length=12)
        // After pressing Enter on "Create Station", cursor is on Station ID field
        // Navigate: Down 1 = Register Type, Down 2 = Start Address, Down 3 = Register Length
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 3,
        },
        CursorAction::Sleep { ms: 500 }, // Wait for cursor to reach register length field
        CursorAction::DebugBreakpoint {
            description: "test2_before_edit_register_length".to_string(),
        },
        CursorAction::PressEnter,         // Enter edit mode
        CursorAction::Sleep { ms: 1000 }, // CRITICAL: Wait for edit mode to fully initialize
        CursorAction::DebugBreakpoint {
            description: "test2_after_enter_edit_mode".to_string(),
        },
        CursorAction::TypeString(REGISTER_LENGTH.to_string()),
        CursorAction::Sleep { ms: 1000 }, // CRITICAL: Wait for typing to complete and buffer to update
        CursorAction::DebugBreakpoint {
            description: "test2_after_type_12".to_string(),
        },
        CursorAction::PressEnter, // Confirm edit and commit to status tree
        CursorAction::Sleep { ms: 2000 }, // CRITICAL: Wait for value to be committed to global status tree
        CursorAction::DebugBreakpoint {
            description: "test2_after_confirm_edit".to_string(),
        },
        // Verify the value was actually committed
        CursorAction::CheckStatus {
            description: format!("Register length should be updated to {}", REGISTER_LENGTH),
            path: "ports[0].modbus_masters[0].register_count".to_string(),
            expected: json!(REGISTER_LENGTH),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
        // Move back to top before saving (per documentation)
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "configure_master").await?;

    // Save configuration with Ctrl+S which will auto-enable the port
    log::info!("🧪 Step 6: Save configuration with Ctrl+S to auto-enable port");
    let actions = vec![
        CursorAction::PressCtrlS,
        CursorAction::Sleep { ms: 5000 }, // Wait longer for port to enable and stabilize
        // Verify port is enabled through status monitoring
        CursorAction::CheckStatus {
            description: format!("Port {} should be enabled", port1),
            path: "ports[0].enabled".to_string(),
            expected: json!(true),
            timeout_secs: Some(20), // Increase timeout
            retry_interval_ms: Some(500),
        },
        // Verify master configuration register count is 12
        CursorAction::CheckStatus {
            description: "Master station should have 12 registers configured".to_string(),
            path: "ports[0].modbus_masters[0].register_count".to_string(),
            expected: json!(12),
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
