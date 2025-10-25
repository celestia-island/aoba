/// TUI E2E tests for multi-station (2 stations) Slave mode configurations
///
/// Tests TUI acting as Modbus Slave with multiple stations configured.
use anyhow::{anyhow, Result};

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::{generate_random_coils, generate_random_registers},
    helpers::sleep_seconds,
    key_input::ArrowKey,
    ports::{port_exists, vcom_matchers_with_ports},
    snapshot::{TerminalCapture, TerminalSize},
    terminal::spawn_expect_process,
    tui::enter_modbus_panel,
};
use serde_json::json;

/// Helper to configure multiple TUI Slave stations following CLAUDE.md workflow
async fn configure_multiple_tui_slave_stations<T: expectrl::Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    stations: &[(u8, &str, u16, u16)], // (id, mode, addr, count)
) -> Result<()> {
    log::info!("🔧 Configuring {} TUI Slave stations", stations.len());

    // Phase 1: Create all stations first
    for i in 0..stations.len() {
        let actions = vec![
            CursorAction::PressEnter, // Create station
            CursorAction::Sleep { ms: 1000 },
            CursorAction::PressCtrlPageUp, // Return to Create Station button
        ];
        execute_cursor_actions(session, cap, &actions, &format!("create_station_{}", i + 1))
            .await?;
    }

    // Configure Connection Mode to Slave (for first station, applies to all)
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::PressEnter,
        CursorAction::PressArrow {
            direction: ArrowKey::Left,
            count: 1,
        },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressCtrlPageUp,
    ];
    execute_cursor_actions(session, cap, &actions, "set_slave_mode").await?;

    // Phase 2: Configure each station
    for (i, (station_id, register_mode, start_address, register_count)) in
        stations.iter().enumerate()
    {
        log::info!(
            "🔧 Configuring slave station {}: ID={}, mode={}, addr=0x{:04X}, count={}",
            i + 1,
            station_id,
            register_mode,
            start_address,
            register_count
        );

        // Navigate to station using Ctrl+PgUp + PgDown
        let mut actions = vec![CursorAction::PressCtrlPageUp];
        for _ in 0..=i {
            actions.push(CursorAction::PressPageDown);
        }
        execute_cursor_actions(session, cap, &actions, &format!("nav_to_station_{}", i + 1))
            .await?;

        // Configure Station ID
        let actions = vec![
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1,
            },
            CursorAction::PressEnter,
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString(station_id.to_string()),
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 200 },
        ];
        execute_cursor_actions(session, cap, &actions, &format!("station_{}_id", i + 1)).await?;

        // Skip Connection Mode (already set to Slave)
        let actions = vec![CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        }];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("station_{}_skip_mode", i + 1),
        )
        .await?;

        // Configure Register Type
        let register_mode_navigation = match *register_mode {
            "coils" => vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::PressEnter,
                CursorAction::PressArrow {
                    direction: ArrowKey::Left,
                    count: 2,
                },
                CursorAction::PressEnter,
            ],
            "discrete_inputs" => vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::PressEnter,
                CursorAction::PressArrow {
                    direction: ArrowKey::Left,
                    count: 1,
                },
                CursorAction::PressEnter,
            ],
            "holding" => vec![CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1,
            }],
            "input" => vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::PressEnter,
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: 1,
                },
                CursorAction::PressEnter,
            ],
            _ => return Err(anyhow!("Invalid register mode: {}", register_mode)),
        };
        execute_cursor_actions(
            session,
            cap,
            &register_mode_navigation,
            &format!("station_{}_type", i + 1),
        )
        .await?;

        // Configure Start Address
        let actions = vec![
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1,
            },
            CursorAction::PressEnter,
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString(format!("{:x}", start_address)),
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 200 },
        ];
        execute_cursor_actions(session, cap, &actions, &format!("station_{}_addr", i + 1)).await?;

        // Configure Register Count
        let actions = vec![
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1,
            },
            CursorAction::PressEnter,
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString(register_count.to_string()),
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 200 },
        ];
        execute_cursor_actions(session, cap, &actions, &format!("station_{}_count", i + 1)).await?;

        // Return to top for next station
        let actions = vec![
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep { ms: 300 },
        ];
        execute_cursor_actions(session, cap, &actions, &format!("return_top_{}", i + 1)).await?;
    }

    log::info!("✅ All {} slave stations configured", stations.len());
    Ok(())
}

/// Test: Mixed Register Types - Station 1 WritableCoils, Station 2 WritableRegisters
/// Both stations: ID=1, addr=0x0000, len=10
pub async fn test_tui_multi_slave_mixed_register_types(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Multi-Slave Test: Mixed Register Types");
    log::info!("  Station 1: Discrete Inputs (writable coils) mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Input (writable registers) mode (ID=1, addr=0x0000, len=10)");

    let ports = vcom_matchers_with_ports(port1, port2);

    if !port_exists(&ports.port1_name) || !port_exists(&ports.port2_name) {
        return Err(anyhow!("Virtual COM ports not available"));
    }

    let station1_data = generate_random_coils(10);
    let station2_data = generate_random_registers(10);
    log::info!("🎲 Station 1 data (coils): {:?}", station1_data);
    log::info!("🎲 Station 2 data (registers): {:?}", station2_data);

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

    let stations = vec![
        (1u8, "discrete_inputs", 0x0000u16, 10u16),
        (1u8, "input", 0x0000u16, 10u16),
    ];
    configure_multiple_tui_slave_stations(&mut tui_session, &mut tui_cap, &stations).await?;

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

    log::info!("✅ Multi-Slave Mixed Register Types test completed successfully");
    drop(tui_session);
    Ok(())
}

/// Test: Spaced Addresses - Station 1 at 0x0000, Station 2 at 0x00A0
/// Both stations: Holding mode, ID=1, len=10
pub async fn test_tui_multi_slave_spaced_addresses(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Multi-Slave Test: Spaced Addresses");
    log::info!("  Station 1: Holding mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=1, addr=0x00A0, len=10)");

    let ports = vcom_matchers_with_ports(port1, port2);

    if !port_exists(&ports.port1_name) || !port_exists(&ports.port2_name) {
        return Err(anyhow!("Virtual COM ports not available"));
    }

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

    let stations = vec![
        (1u8, "holding", 0x0000u16, 10u16),
        (1u8, "holding", 0x00A0u16, 10u16),
    ];
    configure_multiple_tui_slave_stations(&mut tui_session, &mut tui_cap, &stations).await?;

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

    log::info!("✅ Multi-Slave Spaced Addresses test completed successfully");
    drop(tui_session);
    Ok(())
}

/// Test: Mixed Station IDs - Station ID=2 and Station ID=6
/// Both stations: Holding mode, addr=0x0000, len=10
pub async fn test_tui_multi_slave_mixed_station_ids(port1: &str, port2: &str) -> Result<()> {
    log::info!("🧪 Starting TUI Multi-Slave Test: Mixed Station IDs");
    log::info!("  Station 1: Holding mode (ID=2, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=6, addr=0x0000, len=10)");

    let ports = vcom_matchers_with_ports(port1, port2);

    if !port_exists(&ports.port1_name) || !port_exists(&ports.port2_name) {
        return Err(anyhow!("Virtual COM ports not available"));
    }

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

    let stations = vec![
        (2u8, "holding", 0x0000u16, 10u16),
        (6u8, "holding", 0x0000u16, 10u16),
    ];
    configure_multiple_tui_slave_stations(&mut tui_session, &mut tui_cap, &stations).await?;

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

    log::info!("✅ Multi-Slave Mixed Station IDs test completed successfully");
    drop(tui_session);
    Ok(())
}
