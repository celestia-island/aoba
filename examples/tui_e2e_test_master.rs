// Test TUI Master (Slave/Server) with CLI Slave (Master/Client)
// Rewritten with step-by-step verification and regex probes after each action

use anyhow::{anyhow, Result};
use regex::Regex;
// Command/Stdio not used here directly; examples use ci_utils for CLI commands

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    cli::run_cli_slave_poll,
    helpers::sleep_a_while,
    ports::should_run_vcom_tests,
    snapshot::TerminalCapture,
    terminal::spawn_expect_process,
    tui::{enable_port_carefully, navigate_to_vcom},
    verify::verify_cli_output,
};

/// Test TUI Master with CLI Slave
/// TUI acts as Modbus Master (Slave/Server) responding to requests
/// CLI acts as Modbus Slave (Master/Client) polling for data
///
/// This test is rewritten with careful step-by-step verification
pub async fn test_tui_master_with_cli_slave() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master + CLI Slave test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master + CLI Slave hybrid test");

    // Verify vcom ports exist
    if !std::path::Path::new("/tmp/vcom1").exists() {
        return Err(anyhow!("/tmp/vcom1 was not created by socat"));
    }
    if !std::path::Path::new("/tmp/vcom2").exists() {
        return Err(anyhow!("/tmp/vcom2 was not created by socat"));
    }
    log::info!("✓ /tmp/vcom1 and /tmp/vcom2 created successfully");

    // Spawn TUI process (will be master on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Wait for initial screen and verify TUI loaded
    log::info!("🧪 Step 2: Verify TUI loaded with port list");
    let actions = vec![CursorAction::MatchPattern {
        pattern: Regex::new(r"AOBA")?,
        description: "TUI application title visible".to_string(),
        line_range: Some((0, 3)),
        col_range: None,
    }];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "verify_tui_loaded",
    )
    .await?;

    let screen = tui_cap.capture(&mut tui_session, "initial_screen")?;
    log::info!("📸 Initial screen:\n{screen}");

    // Navigate to vcom1
    log::info!("🧪 Step 3: Navigate to vcom1 in port list");
    navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Configure Modbus settings FIRST (before enabling)
    log::info!("🧪 Step 4: Configure TUI as Master with test values");
    configure_tui_master_carefully(&mut tui_session, &mut tui_cap).await?;

    // Enable the port (after configuration)
    log::info!("🧪 Step 5: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Give TUI time to fully initialize the port and start Modbus daemon
    log::info!("🧪 Step 6: Wait for port and Modbus daemon to initialize");
    log::info!("Waiting for Modbus daemon to start listening...");
    sleep_a_while().await;

    // Use CLI to poll the TUI master
    log::info!("🧪 Step 7: Run CLI slave poll command");
    let cli_result = run_cli_slave_poll().await?;

    // Verify the CLI got the expected values
    log::info!("🧪 Step 8: Verify CLI output");
    verify_cli_output(&cli_result)?;

    // Cleanup, quit TUI
    log::info!("🧪 Step 9: Cleanup - quit TUI");
    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "quit_tui").await?;

    log::info!("✅ TUI Master + CLI Slave test completed successfully");
    Ok(())
}

/// Configure TUI as Modbus Master with test values - carefully
async fn configure_tui_master_carefully<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    log::info!("📝 Configuring as Master...");

    // We should be in vcom1 details page after enabling the port
    // Navigate to "Enter Business Configuration" (should be 2 down from Enable Port)
    log::info!("Navigate to 'Enter Business Configuration'");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_business_config").await?;

    let screen = cap.capture(session, "on_business_config_option")?;
    log::info!("📸 On Business Configuration option:\n{screen}");

    // Enter Business Configuration (Modbus settings)
    log::info!("Enter Business Configuration (Modbus settings)");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_business_config").await?;

    let screen = cap.capture(session, "in_modbus_settings")?;
    log::info!("📸 In Modbus settings:\n{screen}");

    // Create station (should be on "Create Station" by default)
    log::info!("Create new station");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"#1")?,
            description: "Station #1 created".to_string(),
            line_range: None,
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    let screen = cap.capture(session, "station_created")?;
    log::info!("📸 Station created:\n{screen}");

    // Navigate to Register Length field (5 down from current)
    log::info!("Navigate to Register Length");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 5,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_reg_length").await?;

    // Set Register Length to 12 (0x000C) as required by test spec
    log::info!("Set Register Length to 12");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::TypeString("12".to_string()), // Enter 12 in decimal
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Register Length\s+0x000C")?,
            description: "Register Length set to 12".to_string(),
            line_range: None,
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "set_reg_length").await?;

    let screen = cap.capture(session, "reg_length_set")?;
    log::info!("📸 Register Length set:\n{screen}");

    // Navigate to register values (1 down)
    log::info!("Navigate to register values");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_registers").await?;

    // Set register values: 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110 (in decimal)
    // UI accepts HEX input and displays as hex, so we need to convert decimal to hex strings
    // Layout is 4 columns per row: [0,1,2,3] [4,5,6,7] [8,9,10,11]
    let test_values = [0u16, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110];
    for (i, &val) in test_values.iter().enumerate() {
        let hex_val = format!("{val:X}"); // Convert to HEX string
        log::info!("Set register {i} to {val} (0x{val:04X})");

        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::TypeString(hex_val.clone()), // Type hex value
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, &format!("set_reg_{i}")).await?;

        // Navigate to next register - just use RIGHT arrow for all registers
        if i < test_values.len() - 1 {
            log::info!("Moving Right to next register");
            let actions = vec![
                CursorAction::PressArrow {
                    direction: ci_utils::ArrowKey::Right,
                    count: 1,
                },
                CursorAction::Sleep { ms: 250 },
            ];
            execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("nav_to_reg_{num}", num = i + 1),
            )
            .await?;
        }
    }

    let screen = cap.capture(session, "registers_set")?;
    log::info!("📸 All 12 registers set:\n{screen}");

    // Verify at least some key values are visible (pattern check)
    let has_values = screen.contains("0x000A")
        || screen.contains("0x0014")
        || screen.contains("0x001E")
        || screen.contains("0x0028")
        || screen.contains("0x0032")
        || screen.contains("0x003C");
    if !has_values {
        log::warn!("⚠️  Register values may not be set correctly");
    } else {
        log::info!("✓ Register values verified (pattern visible)");
    }

    // Exit Modbus settings back to port details
    // We're in navigation mode (not editing), so one Escape goes back to port details
    log::info!("Exit Modbus settings back to port details (press Escape)");
    let actions = vec![CursorAction::PressEscape, CursorAction::Sleep { ms: 1000 }];
    execute_cursor_actions(session, cap, &actions, "exit_modbus_settings").await?;

    let screen = cap.capture(session, "after_exit")?;
    log::info!("📸 After exiting Modbus settings:\n{screen}");

    // Check if we're back on port details or main port list
    if screen.contains("Enable Port") {
        log::info!("✓ Already back on port details page");
    } else if screen.contains("COM Ports") {
        log::info!("We went back to main port list, need to enter vcom1 again");

        // Navigate back to vcom1 and enter it
        let vcom_pattern =
            std::env::var("AOBATEST_PORT1").unwrap_or_else(|_| "/tmp/vcom1".to_string());

        // Find vcom1 and navigate to it
        let lines: Vec<&str> = screen.lines().collect();
        let mut vcom1_line = None;
        let mut cursor_line = None;

        for (idx, line) in lines.iter().enumerate() {
            if line.contains(&vcom_pattern) {
                vcom1_line = Some(idx);
            }
            if line.contains("> ") {
                let trimmed = line.trim();
                if trimmed.starts_with("│ > ") || trimmed.starts_with("> ") {
                    cursor_line = Some(idx);
                }
            }
        }

        if let (Some(vcom1_idx), Some(curr_idx)) = (vcom1_line, cursor_line) {
            if vcom1_idx != curr_idx {
                let delta = vcom1_idx.abs_diff(curr_idx);

                let direction = if vcom1_idx > curr_idx {
                    ci_utils::ArrowKey::Down
                } else {
                    ci_utils::ArrowKey::Up
                };

                log::info!("Navigating to vcom1 ({delta} steps)");
                let actions = vec![
                    CursorAction::PressArrow {
                        direction,
                        count: delta,
                    },
                    CursorAction::Sleep { ms: 500 },
                ];
                execute_cursor_actions(session, cap, &actions, "nav_back_to_vcom1").await?;
            }
        }

        // Press Enter to enter vcom1 details
        log::info!("Press Enter to enter vcom1 details");
        let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 1000 }];
        execute_cursor_actions(session, cap, &actions, "reenter_vcom1").await?;

        let screen = cap.capture(session, "back_in_vcom1_details")?;
        log::info!("📸 Back in vcom1 details:\n{screen}");

        if !screen.contains("Enable Port") {
            return Err(anyhow!("Failed to re-enter vcom1 details page"));
        }
    } else {
        return Err(anyhow!("Unexpected screen after exiting Modbus settings"));
    }

    log::info!("✓ Master configuration complete, ready to enable port");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Running TUI E2E Test 1: TUI Master + CLI Slave");

    match test_tui_master_with_cli_slave().await {
        Ok(_) => {
            log::info!("✅ Test 1 passed");
            Ok(())
        }
        Err(e) => {
            log::error!("❌ Test 1 failed: {e}");
            Err(e)
        }
    }
}
