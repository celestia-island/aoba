/// TUI E2E tests for single-station Master mode with different register modes
///
/// Tests TUI acting as Modbus Master (server) with E2E process as CLI Slave (client).
/// Each test follows the detailed workflow from CLAUDE.md for TUI configuration.
use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::{generate_random_coils, generate_random_registers},
    helpers::sleep_seconds,
    key_input::ArrowKey,
    ports::{port_exists, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2},
    snapshot::{TerminalCapture, TerminalSize},
    terminal::{build_debug_bin, spawn_expect_process},
    tui::enter_modbus_panel,
};
use serde_json::json;

/// Helper to configure a TUI station with specified parameters
/// This follows the detailed workflow from CLAUDE.md
async fn configure_tui_station<T: expectrl::Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_id: u8,
    register_mode: &str,  // "coils", "discrete_inputs", "holding", "input"
    start_address: u16,
    register_count: u16,
    register_values: Option<&[u16]>,
) -> Result<()> {
    log::info!(
        "🔧 Configuring TUI station: ID={}, mode={}, addr=0x{:04X}, count={}",
        station_id,
        register_mode,
        start_address,
        register_count
    );

    // Step 1: Create station by pressing Enter on "Create Station"
    let actions = vec![
        CursorAction::PressEnter,        // Create station
        CursorAction::Sleep { ms: 1000 },
    ];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    // Step 2: Configure Station ID (cursor is already on Station ID field after creation)
    let actions = vec![
        CursorAction::PressEnter,        // Enter edit mode
        CursorAction::PressCtrlA,        // Select all
        CursorAction::PressBackspace,    // Clear
        CursorAction::TypeString(station_id.to_string()),
        CursorAction::PressEnter,        // Confirm
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "configure_station_id").await?;

    // Step 3: Configure Register Type (now on Register Type field)
    // Default is "Holding" (index 2), so we need to navigate based on desired mode
    // Modes: 0=Coils, 1=DiscreteInputs, 2=Holding, 3=Input
    let register_mode_navigation = match register_mode {
        "coils" => vec![
            CursorAction::PressEnter,
            CursorAction::PressArrow {
                direction: ArrowKey::Left,
                count: 2,
            },
            CursorAction::PressEnter,
        ],
        "discrete_inputs" => vec![
            CursorAction::PressEnter,
            CursorAction::PressArrow {
                direction: ArrowKey::Left,
                count: 1,
            },
            CursorAction::PressEnter,
        ],
        "holding" => vec![
            // Already at default, no navigation needed
        ],
        "input" => vec![
            CursorAction::PressEnter,
            CursorAction::PressArrow {
                direction: ArrowKey::Right,
                count: 1,
            },
            CursorAction::PressEnter,
        ],
        _ => return Err(anyhow!("Invalid register mode: {}", register_mode)),
    };

    let mut actions = register_mode_navigation;
    actions.push(CursorAction::Sleep { ms: 200 });
    actions.push(CursorAction::PressArrow {
        direction: ArrowKey::Down,
        count: 1,
    });
    execute_cursor_actions(session, cap, &actions, "configure_register_type").await?;

    // Step 4: Configure Start Address (now on Start Address field)
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(format!("{:x}", start_address)), // Hex without 0x prefix
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "configure_start_address").await?;

    // Step 5: Configure Register Count (now on Register Length field)
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(register_count.to_string()), // Decimal
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "configure_register_count").await?;

    // Step 6: Configure individual register values if provided
    if let Some(values) = register_values {
        log::info!("🔧 Configuring {} register values", values.len());
        
        for (i, &value) in values.iter().enumerate() {
            let actions = vec![
                CursorAction::PressEnter,        // Enter edit mode
                CursorAction::TypeString(format!("{:x}", value)), // Hex without 0x prefix
                CursorAction::PressEnter,        // Confirm
                CursorAction::Sleep { ms: 100 },
                // Verify value was written to status tree
                CursorAction::CheckStatus {
                    description: format!("Register {} value committed", i),
                    path: format!("ports[0].modbus_masters[0].registers[{}]", i),
                    expected: json!(value),
                    timeout_secs: Some(5),
                    retry_interval_ms: Some(300),
                },
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: 1,
                }, // Move to next register
            ];
            execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("set_register_{}", i),
            )
            .await?;
        }
    }

    // Step 7: Return to top of panel with Ctrl+PgUp
    let actions = vec![
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(session, cap, &actions, "return_to_top").await?;

    log::info!("✅ Station configuration completed");
    Ok(())
}

/// Test 01: TUI Master with Coils mode (0x0000, length 10)
pub async fn test_tui_master_coils(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Master Single-Station Test: 01 Coils Mode");

    let ports = vcom_matchers_with_ports(port1, port2);

    // Verify ports exist
    if !port_exists(&ports.port1_name) || !port_exists(&ports.port2_name) {
        return Err(anyhow!("Virtual COM ports not available"));
    }

    // Generate test data
    let test_data = generate_random_coils(10);
    log::info!("🎲 Test data: {:?}", test_data);

    // TODO: Step 1 - Spawn TUI process in debug mode
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);

    sleep_seconds(3).await;

    // TODO: Step 2 - Wait for TUI to reach Entry page
    log::info!("🧪 Step 2: Wait for Entry page");
    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should be on Entry page".to_string(),
        path: "page.type".to_string(),
        expected: json!("Entry"),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "wait_entry").await?;

    // TODO: Step 3 - Navigate to port and enter ConfigPanel
    log::info!("🧪 Step 3: Navigate to port {}", port1);
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        CursorAction::CheckStatus {
            description: "Should be on ConfigPanel".to_string(),
            path: "page.type".to_string(),
            expected: json!("ConfigPanel"),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "enter_config").await?;

    // TODO: Step 4 - Enter Modbus configuration panel
    log::info!("🧪 Step 4: Enter Modbus panel");
    enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;

    // TODO: Step 5 - Verify clean state
    log::info!("🧪 Step 5: Verify clean state");
    let actions = vec![
        CursorAction::CheckStatus {
            description: "Port should be disabled".to_string(),
            path: "ports[0].enabled".to_string(),
            expected: json!(false),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_clean").await?;

    // TODO: Step 6 - Configure as Master with Coils mode
    log::info!("🧪 Step 6: Configure Master station");
    configure_tui_station(
        &mut tui_session,
        &mut tui_cap,
        1,           // station_id
        "coils",     // register_mode
        0x0000,      // start_address
        10,          // register_count
        Some(&test_data), // register_values
    )
    .await?;

    // TODO: Step 7 - Save configuration with Ctrl+S (this enables the port)
    log::info!("🧪 Step 7: Save configuration and enable port");
    let actions = vec![
        CursorAction::PressCtrlS,
        CursorAction::Sleep { ms: 5000 }, // Wait for port to enable
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_config").await?;

    // TODO: Step 8 - Verify port is enabled
    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    // TODO: Step 9 - Spawn CLI Slave to verify communication
    log::info!("🧪 Step 9: Spawn CLI Slave to verify data");
    let binary = build_debug_bin("aoba")?;
    let slave_output = Command::new(&binary)
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-mode",
            "coils",
            "--register-address",
            "0",
            "--register-length",
            "10",
            "--baud-rate",
            "9600",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    // TODO: Step 10 - Verify CLI Slave received correct data
    if !slave_output.status.success() {
        let stderr = String::from_utf8_lossy(&slave_output.stderr);
        return Err(anyhow!("CLI Slave failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&slave_output.stdout);
    log::info!("CLI Slave output: {}", stdout);

    // TODO: Parse JSON and verify data
    let received_data: Vec<u16> = serde_json::from_str(stdout.trim())?;
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);
        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    // Cleanup
    drop(tui_session);

    log::info!("✅ TUI Master Coils Mode test completed successfully");
    Ok(())
}

/// Test 02: TUI Master with Discrete Inputs/Writable Coils mode (0x0010, length 10)
pub async fn test_tui_master_discrete_inputs(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Master Single-Station Test: 02 Discrete Inputs Mode");

    // TODO: Similar implementation to test_tui_master_coils
    // with bidirectional write testing

    log::warn!("⚠️ Test not yet fully implemented - TODO");
    Ok(())
}

/// Test 03: TUI Master with Holding Registers mode (0x0020, length 10)
pub async fn test_tui_master_holding_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Master Single-Station Test: 03 Holding Registers Mode");

    // TODO: Similar implementation to test_tui_master_coils
    // but with holding registers instead of coils

    log::warn!("⚠️ Test not yet fully implemented - TODO");
    Ok(())
}

/// Test 04: TUI Master with Input Registers/Writable Registers mode (0x0030, length 10)
pub async fn test_tui_master_input_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Master Single-Station Test: 04 Input Registers Mode");

    // TODO: Similar implementation to test_tui_master_coils
    // with bidirectional write testing

    log::warn!("⚠️ Test not yet fully implemented - TODO");
    Ok(())
}
