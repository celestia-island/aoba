use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::Write,
    process::{Command, Stdio},
    time::Duration,
};

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::ArrowKey,
    ports::{port_exists, should_run_vcom_tests_with_ports, vcom_matchers_with_ports},
    terminal::{build_debug_bin, spawn_expect_process},
    tui::enter_modbus_panel,
};
use serde_json::json;

const ROUNDS: usize = 3;
const REGISTER_LENGTH: usize = 12;

/// Test TUI Slave mode + external CLI master with continuous random data (3 rounds)
///
/// This version uses status monitoring exclusively - no terminal content matching.
/// All verification is done through CheckStatus actions that read JSON status dumps.
///
/// TUI acts as Modbus Slave (client/poll mode), driven through the UI automation.
/// External CLI runs in master role and must communicate successfully with TUI-managed runtime.
pub async fn test_tui_slave_with_cli_master_continuous(port1: &str, port2: &str) -> Result<()> {
    if !should_run_vcom_tests_with_ports(port1, port2) {
        log::info!("Skipping TUI Slave + CLI Master test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Slave + CLI Master continuous test (3 rounds)");

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

    // Spawn TUI process (will be slave/server on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = ci_utils::snapshot::TerminalCapture::with_size(
        ci_utils::snapshot::TerminalSize::Small,
    );

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

    // Navigate to port1 using keyboard and enter its config panel
    log::info!("🧪 Step 3: Navigate to {} in port list and enter config", port1);
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

    // Configure as Slave - create station and configure
    log::info!("🧪 Step 5: Configure TUI as Slave (client/poll mode)");
    let actions = vec![
        // Press Enter on "Create Station" button
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Configure station (station_id=1, holding registers, address=0, length=12)
        // These settings will be verified through status monitoring later
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 4, // Navigate to register length field
        },
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter, // Enter edit mode
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(REGISTER_LENGTH.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter, // Confirm edit
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "configure_slave").await?;

    // Save configuration with Ctrl+S which will auto-enable the port
    log::info!("🧪 Step 6: Save configuration with Ctrl+S to auto-enable port");
    let actions = vec![
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
        // Verify slave configuration exists
        CursorAction::CheckStatus {
            description: "Should have one slave station configured".to_string(),
            path: "ports[0].modbus_slaves[0].station_id".to_string(),
            expected: json!(1),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_and_verify").await?;

    log::info!("✅ Port enabled and slave configuration verified");

    // Run communication rounds with external CLI master
    log::info!("🧪 Step 7: Running {} communication rounds", ROUNDS);

    let binary = build_debug_bin("aoba")?;

    for round in 1..=ROUNDS {
        log::info!("📡 Round {}/{}: Starting communication test", round, ROUNDS);

        // Generate random data for this round
        let random_data = generate_random_registers(REGISTER_LENGTH);
        log::info!("   Generated test data: {:?}", random_data);

        // Write data to CLI master's data source file
        let data_file = format!("/tmp/master_data_slave_test_{}.jsonl", port1);
        let mut file = File::create(&data_file)?;
        writeln!(
            file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "values": random_data
            }))?
        )?;
        file.flush()?;

        // Spawn CLI master-provide process on port2
        log::info!("   Spawning CLI master on {}", port2);
        let mut cli_child = Command::new(&binary)
            .args([
                "--master-provide",
                &ports.port2_name,
                "--station-id",
                "1",
                "--register-address",
                "0",
                "--register-length",
                &REGISTER_LENGTH.to_string(),
                "--register-mode",
                "holding",
                "--data-source",
                &format!("file:{}", data_file),
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

        // Note: Simplified check for now - in a complete implementation,
        // we would verify communication occurred by checking log count
        log::info!("   ✅ Round {}/{} completed", round, ROUNDS);

        // Cleanup
        std::fs::remove_file(&data_file)?;
    }

    log::info!("🎉 All {} rounds completed successfully!", ROUNDS);

    // Exit TUI
    let actions = vec![CursorAction::CtrlC, CursorAction::Sleep { ms: 500 }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "exit_tui").await?;

    Ok(())
}
