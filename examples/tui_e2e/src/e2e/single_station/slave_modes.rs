/// TUI E2E tests for single-station Slave mode with different register modes
///
/// Tests TUI acting as Modbus Slave with E2E process as CLI Master.
/// Each test follows the detailed workflow from CLAUDE.md for TUI configuration.
use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::{generate_random_coils, generate_random_registers},
    helpers::sleep_seconds,
    key_input::ArrowKey,
    ports::{port_exists, vcom_matchers_with_ports},
    snapshot::{TerminalCapture, TerminalSize},
    terminal::{build_debug_bin, spawn_expect_process},
    tui::enter_modbus_panel,
};
use serde_json::json;

/// Helper to configure a TUI station in Slave mode
async fn configure_tui_slave_station<T: expectrl::Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_id: u8,
    register_mode: &str,
    start_address: u16,
    register_count: u16,
) -> Result<()> {
    log::info!(
        "🔧 Configuring TUI slave station: ID={}, mode={}, addr=0x{:04X}, count={}",
        station_id,
        register_mode,
        start_address,
        register_count
    );

    // Create station
    let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 1000 }];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    // Configure Station ID
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(station_id.to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "configure_station_id").await?;

    // Configure Connection Mode to Slave
    let actions = vec![
        CursorAction::PressEnter, // Enter edit mode
        CursorAction::PressArrow {
            direction: ArrowKey::Left, // Move from Master (0) to Slave (1)
            count: 1,
        },
        CursorAction::PressEnter, // Confirm
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "configure_slave_mode").await?;

    // Configure Register Type
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
        "holding" => vec![],
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

    // Configure Start Address
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(format!("{:x}", start_address)),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "configure_start_address").await?;

    // Configure Register Count
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(register_count.to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
    ];
    execute_cursor_actions(session, cap, &actions, "configure_register_count").await?;

    // Return to top
    let actions = vec![
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(session, cap, &actions, "return_to_top").await?;

    log::info!("✅ Slave station configuration completed");
    Ok(())
}

/// Test 01: TUI Slave with Coils mode (0x0000, length 10)
pub async fn test_tui_slave_coils(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Slave Single-Station Test: 01 Coils Mode");

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

    configure_tui_slave_station(&mut tui_session, &mut tui_cap, 1, "coils", 0x0000, 10).await?;

    let actions = vec![CursorAction::PressCtrlS, CursorAction::Sleep { ms: 5000 }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_config").await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    // Now spawn CLI Master to send data to TUI Slave
    let binary = build_debug_bin("aoba")?;
    let master_output = Command::new(&binary)
        .args([
            "--master-provide",
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
            "--data-source",
            &format!("inline:{}", serde_json::to_string(&test_data)?),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !master_output.status.success() {
        let stderr = String::from_utf8_lossy(&master_output.stderr);
        return Err(anyhow!("CLI Master failed: {}", stderr));
    }

    log::info!("✅ CLI Master successfully sent data");

    // Wait for TUI to receive and process data
    sleep_seconds(2).await;

    // Verify TUI received the data by checking status
    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should have received data".to_string(),
        path: "ports[0].modbus_slaves[0].registers".to_string(),
        expected: json!(test_data),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_data").await?;

    log::info!("✅ Data verified successfully!");

    drop(tui_session);

    log::info!("✅ TUI Slave Coils Mode test completed successfully");
    Ok(())
}

/// Test 02: TUI Slave with Discrete Inputs/Writable Coils mode (0x0010, length 10)
pub async fn test_tui_slave_discrete_inputs(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Slave Single-Station Test: 02 Discrete Inputs Mode");

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

    configure_tui_slave_station(
        &mut tui_session,
        &mut tui_cap,
        1,
        "discrete_inputs",
        0x0010,
        10,
    )
    .await?;

    let actions = vec![CursorAction::PressCtrlS, CursorAction::Sleep { ms: 5000 }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_config").await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    let binary = build_debug_bin("aoba")?;
    let master_output = Command::new(&binary)
        .args([
            "--master-provide",
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
            "--data-source",
            &format!("inline:{}", serde_json::to_string(&test_data)?),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !master_output.status.success() {
        let stderr = String::from_utf8_lossy(&master_output.stderr);
        return Err(anyhow!("CLI Master failed: {}", stderr));
    }

    log::info!("✅ CLI Master successfully sent data");
    sleep_seconds(2).await;

    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should have received data".to_string(),
        path: "ports[0].modbus_slaves[0].registers".to_string(),
        expected: json!(test_data),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_data").await?;

    log::info!("✅ Data verified successfully!");

    drop(tui_session);

    log::info!("✅ TUI Slave Discrete Inputs Mode test completed successfully");
    Ok(())
}

/// Test 03: TUI Slave with Holding Registers mode (0x0020, length 10)
pub async fn test_tui_slave_holding_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Slave Single-Station Test: 03 Holding Registers Mode");

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

    configure_tui_slave_station(&mut tui_session, &mut tui_cap, 1, "holding", 0x0020, 10).await?;

    let actions = vec![CursorAction::PressCtrlS, CursorAction::Sleep { ms: 5000 }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_config").await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    let binary = build_debug_bin("aoba")?;
    let master_output = Command::new(&binary)
        .args([
            "--master-provide",
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
            "--data-source",
            &format!("inline:{}", serde_json::to_string(&test_data)?),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !master_output.status.success() {
        let stderr = String::from_utf8_lossy(&master_output.stderr);
        return Err(anyhow!("CLI Master failed: {}", stderr));
    }

    log::info!("✅ CLI Master successfully sent data");
    sleep_seconds(2).await;

    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should have received data".to_string(),
        path: "ports[0].modbus_slaves[0].registers".to_string(),
        expected: json!(test_data),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_data").await?;

    log::info!("✅ Data verified successfully!");

    drop(tui_session);

    log::info!("✅ TUI Slave Holding Registers Mode test completed successfully");
    Ok(())
}

/// Test 04: TUI Slave with Input Registers/Writable Registers mode (0x0030, length 10)
pub async fn test_tui_slave_input_registers(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Slave Single-Station Test: 04 Input Registers Mode");

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

    configure_tui_slave_station(&mut tui_session, &mut tui_cap, 1, "input", 0x0030, 10).await?;

    let actions = vec![CursorAction::PressCtrlS, CursorAction::Sleep { ms: 5000 }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_config").await?;

    let actions = vec![CursorAction::CheckStatus {
        description: "Port should be enabled".to_string(),
        path: "ports[0].enabled".to_string(),
        expected: json!(true),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_enabled").await?;

    let binary = build_debug_bin("aoba")?;
    let master_output = Command::new(&binary)
        .args([
            "--master-provide",
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
            "--data-source",
            &format!("inline:{}", serde_json::to_string(&test_data)?),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !master_output.status.success() {
        let stderr = String::from_utf8_lossy(&master_output.stderr);
        return Err(anyhow!("CLI Master failed: {}", stderr));
    }

    log::info!("✅ CLI Master successfully sent data");
    sleep_seconds(2).await;

    let actions = vec![CursorAction::CheckStatus {
        description: "TUI should have received data".to_string(),
        path: "ports[0].modbus_slaves[0].registers".to_string(),
        expected: json!(test_data),
        timeout_secs: Some(10),
        retry_interval_ms: Some(500),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "verify_data").await?;

    log::info!("✅ Data verified successfully!");

    drop(tui_session);

    log::info!("✅ TUI Slave Input Registers Mode test completed successfully");
    Ok(())
}
