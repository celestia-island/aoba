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
/// This follows the detailed workflow from CLAUDE.md and problem statement
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
        "🔧 Configuring TUI Master station: ID={}, mode={}, addr=0x{:04X}, count={}",
        station_id,
        register_mode,
        start_address,
        register_count
    );

    // Phase 1: Create station by pressing Enter on "Create Station"
    log::info!("📍 Phase 1: Creating station");
    let actions = vec![
        CursorAction::PressEnter,        // Create station
        CursorAction::Sleep { ms: 2000 }, // Wait longer for station to be created
    ];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    // Verify station was created by checking for "#1" in screen
    use regex::Regex;
    let station_pattern = Regex::new(r"#1(?:\D|$)")?;
    let actions = vec![
        CursorAction::MatchPattern {
            pattern: station_pattern,
            description: "Station #1 exists".to_string(),
            line_range: None,
            col_range: None,
            retry_action: None,
        },
        CursorAction::PressCtrlPageUp,   // Return to top after verifying
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(session, cap, &actions, "verify_station_created").await?;

    // Phase 2: Configure connection mode to Master (default is already Master, so just move down once)
    log::info!("📍 Phase 2: Confirming Master mode (default)");
    let actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
        CursorAction::Sleep { ms: 200 },
        // Master is default, no need to change anything
        CursorAction::PressCtrlPageUp,   // Move to top
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(session, cap, &actions, "confirm_master_mode").await?;

    // Phase 3: Configure station fields using absolute positioning
    log::info!("📍 Phase 3: Configuring station fields");
    
    // Navigate to station #1 (Ctrl+PgUp + PgDown once)
    let actions = vec![
        CursorAction::PressCtrlPageUp,   // Ensure at top
        CursorAction::PressPageDown,     // Navigate to station #1
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(session, cap, &actions, "navigate_to_station").await?;

    // Configure Station ID (field 0, so press Down once to get to it)
    log::info!("🔧 Configuring Station ID: {}", station_id);
    let actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,        // Enter edit mode
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressCtrlA,        // Select all
        CursorAction::PressBackspace,    // Clear
        CursorAction::TypeString(station_id.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEscape,       // Use Escape to exit edit mode
        CursorAction::Sleep { ms: 500 }, // Wait for value to commit
    ];
    execute_cursor_actions(session, cap, &actions, "configure_station_id").await?;

    // Configure Register Type (field 1, press Down once from Station ID)
    log::info!("🔧 Configuring Register Type: {}", register_mode);
    // Default is "Holding" (index 2), navigate based on desired mode
    // Modes: 0=Coils, 1=DiscreteInputs, 2=Holding, 3=Input
    let register_mode_navigation = match register_mode {
        "coils" => vec![
            CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow { direction: ArrowKey::Left, count: 2 },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEscape,     // Use Escape to exit edit mode
            CursorAction::Sleep { ms: 500 },
        ],
        "discrete_inputs" => vec![
            CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow { direction: ArrowKey::Left, count: 1 },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEscape,
            CursorAction::Sleep { ms: 500 },
        ],
        "holding" => vec![
            CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
            CursorAction::Sleep { ms: 500 },
            // Already at default, no need to enter edit mode
        ],
        "input" => vec![
            CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow { direction: ArrowKey::Right, count: 1 },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEscape,
            CursorAction::Sleep { ms: 500 },
        ],
        _ => return Err(anyhow!("Invalid register mode: {}", register_mode)),
    };
    execute_cursor_actions(session, cap, &register_mode_navigation, "configure_register_type").await?;

    // Configure Start Address (field 2, press Down once from Register Type)
    log::info!("🔧 Configuring Start Address: 0x{:04X}", start_address);
    let actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(format!("{:x}", start_address)), // Hex without 0x prefix
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,        // Press Enter to confirm value
        CursorAction::Sleep { ms: 1000 }, // Wait for value to commit
    ];
    execute_cursor_actions(session, cap, &actions, "configure_start_address").await?;

    // Configure Register Count (field 3, press Down once from Start Address)
    // CRITICAL: Must use Enter and wait 2s for value to commit before any navigation
    log::info!("🔧 Configuring Register Count: {}", register_count);
    let actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 }, // Wait for edit mode to fully initialize
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(register_count.to_string()), // Decimal format
        CursorAction::Sleep { ms: 1000 }, // Wait for typing to complete
        CursorAction::PressEnter,         // Confirm edit and commit to status tree
        CursorAction::Sleep { ms: 2000 }, // CRITICAL: Wait for value to commit to global status
    ];
    execute_cursor_actions(session, cap, &actions, "configure_register_count").await?;

    // Verify the register count was actually committed to status tree
    let actions = vec![
        CursorAction::CheckStatus {
            description: format!("Register count should be {}", register_count),
            path: "ports[0].modbus_masters[0].register_count".to_string(),
            expected: json!(register_count),
            timeout_secs: Some(10),
            retry_interval_ms: Some(500),
        },
    ];
    execute_cursor_actions(session, cap, &actions, "verify_register_count").await?;

    // Configure individual register values if provided
    if let Some(values) = register_values {
        log::info!("🔧 Configuring {} register values", values.len());
        
        // After setting register count, cursor should be in the register grid area
        // Press Down once to enter the register grid
        let actions = vec![
            CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
            CursorAction::Sleep { ms: 300 },
        ];
        execute_cursor_actions(session, cap, &actions, "enter_register_grid").await?;
        
        for (i, &value) in values.iter().enumerate() {
            log::info!("  🔹 Setting register {} = 0x{:04X}", i, value);
            let actions = vec![
                CursorAction::PressEnter,        // Enter edit mode
                CursorAction::TypeString(format!("{:x}", value)), // Hex without 0x prefix
                CursorAction::PressEnter,        // Confirm
                CursorAction::Sleep { ms: 200 },
                // Note: Register values are not in status tree until port is enabled with Ctrl+S
                // So we skip CheckStatus here and verify after save instead
            ];
            execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("set_register_{}", i),
            )
            .await?;
            
            // Move to next register (unless it's the last one)
            if i < values.len() - 1 {
                let actions = vec![
                    CursorAction::PressArrow { direction: ArrowKey::Right, count: 1 },
                    CursorAction::Sleep { ms: 100 },
                ];
                execute_cursor_actions(session, cap, &actions, &format!("move_to_register_{}", i + 1)).await?;
            }
        }
    }

    // Save configuration with Ctrl+S to commit all changes
    // Note: Must save BEFORE navigating away or changes will be discarded!
    log::info!("📍 Saving configuration with Ctrl+S");
    let actions = vec![
        CursorAction::Sleep { ms: 2000 }, // Wait for all changes to settle
        CursorAction::PressCtrlS,        // Save configuration directly without navigating first
        CursorAction::Sleep { ms: 5000 }, // Wait for port to enable
    ];
    execute_cursor_actions(session, cap, &actions, "save_and_enable").await?;

    log::info!("✅ Station configuration completed and saved");
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

    // TODO: Step 6 - Configure as Master with Coils mode (saves with Ctrl+S)
    log::info!("🧪 Step 6: Configure Master station and save");
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

    // TODO: Step 7 - Verify port is enabled (configuration was saved in Step 6)
    log::info!("🧪 Step 7: Verify port is enabled");
    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    // TODO: Step 8 - Spawn CLI Slave to verify communication
    log::info!("🧪 Step 8: Spawn CLI Slave to verify data");
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

    // TODO: Step 9 - Verify CLI Slave received correct data
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

    let ports = vcom_matchers_with_ports(port1, port2);

    if !port_exists(&ports.port1_name) || !port_exists(&ports.port2_name) {
        return Err(anyhow!("Virtual COM ports not available"));
    }

    let test_data = generate_random_coils(10);
    log::info!("🎲 Test data: {:?}", test_data);

    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);
    sleep_seconds(3).await;

    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should be on Entry page".to_string(),
        path: "page.type".to_string(),
        expected: json!("Entry"),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "wait_entry").await?;

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

    enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be disabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(false),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_clean").await?;

    configure_tui_station(
        &mut tui_session,
        &mut tui_cap,
        1,
        "discrete_inputs",
        0x0010,
        10,
        Some(&test_data),
    )
    .await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    let binary = build_debug_bin("aoba")?;
    let slave_output = Command::new(&binary)
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-mode",
            "discrete_inputs",
            "--register-address",
            "16",
            "--register-length",
            "10",
            "--baud-rate",
            "9600",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !slave_output.status.success() {
        let stderr = String::from_utf8_lossy(&slave_output.stderr);
        return Err(anyhow!("CLI Slave failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&slave_output.stdout);
    log::info!("CLI Slave output: {}", stdout);

    let received_data: Vec<u16> = serde_json::from_str(stdout.trim())?;
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);
        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    drop(tui_session);

    log::info!("✅ TUI Master Discrete Inputs Mode test completed successfully");
    Ok(())
}

/// Test 03: TUI Master with Holding Registers mode (0x0020, length 10)
pub async fn test_tui_master_holding_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Master Single-Station Test: 03 Holding Registers Mode");

    let ports = vcom_matchers_with_ports(port1, port2);

    if !port_exists(&ports.port1_name) || !port_exists(&ports.port2_name) {
        return Err(anyhow!("Virtual COM ports not available"));
    }

    let test_data = generate_random_registers(10);
    log::info!("🎲 Test data: {:?}", test_data);

    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);
    sleep_seconds(3).await;

    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should be on Entry page".to_string(),
        path: "page.type".to_string(),
        expected: json!("Entry"),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "wait_entry").await?;

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

    enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be disabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(false),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_clean").await?;

    configure_tui_station(
        &mut tui_session,
        &mut tui_cap,
        1,
        "holding",
        0x0020,
        10,
        Some(&test_data),
    )
    .await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    let binary = build_debug_bin("aoba")?;
    let slave_output = Command::new(&binary)
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-mode",
            "holding",
            "--register-address",
            "32",
            "--register-length",
            "10",
            "--baud-rate",
            "9600",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !slave_output.status.success() {
        let stderr = String::from_utf8_lossy(&slave_output.stderr);
        return Err(anyhow!("CLI Slave failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&slave_output.stdout);
    log::info!("CLI Slave output: {}", stdout);

    let received_data: Vec<u16> = serde_json::from_str(stdout.trim())?;
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);
        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    drop(tui_session);

    log::info!("✅ TUI Master Holding Registers Mode test completed successfully");
    Ok(())
}

/// Test 04: TUI Master with Input Registers/Writable Registers mode (0x0030, length 10)
pub async fn test_tui_master_input_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Master Single-Station Test: 04 Input Registers Mode");

    let ports = vcom_matchers_with_ports(port1, port2);

    if !port_exists(&ports.port1_name) || !port_exists(&ports.port2_name) {
        return Err(anyhow!("Virtual COM ports not available"));
    }

    let test_data = generate_random_registers(10);
    log::info!("🎲 Test data: {:?}", test_data);

    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);
    sleep_seconds(3).await;

    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should be on Entry page".to_string(),
        path: "page.type".to_string(),
        expected: json!("Entry"),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "wait_entry").await?;

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

    enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be disabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(false),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_clean").await?;

    configure_tui_station(
        &mut tui_session,
        &mut tui_cap,
        1,
        "input",
        0x0030,
        10,
        Some(&test_data),
    )
    .await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    let binary = build_debug_bin("aoba")?;
    let slave_output = Command::new(&binary)
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-mode",
            "input",
            "--register-address",
            "48",
            "--register-length",
            "10",
            "--baud-rate",
            "9600",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !slave_output.status.success() {
        let stderr = String::from_utf8_lossy(&slave_output.stderr);
        return Err(anyhow!("CLI Slave failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&slave_output.stdout);
    log::info!("CLI Slave output: {}", stdout);

    let received_data: Vec<u16> = serde_json::from_str(stdout.trim())?;
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);
        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    drop(tui_session);

    log::info!("✅ TUI Master Input Registers Mode test completed successfully");
    Ok(())
}
